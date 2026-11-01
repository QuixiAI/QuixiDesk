//! Voice: text-to-speech and speech-to-text that do not go through the webview.
//!
//! QuixiDesk's read-aloud and dictation call the standard `speechSynthesis` / `SpeechRecognition`
//! web APIs and rely on the system webview to implement them. macOS's WKWebView provides Apple's
//! speech stack; **WebKitGTK implements none of them** — `speechSynthesis`, `SpeechSynthesisUtterance`,
//! `SpeechRecognition` and `webkitSpeechRecognition` are all `undefined` on Linux, so both features
//! are dead there regardless of desktop environment.
//!
//! This module bypasses the webview entirely: the engine talks to speech backends over HTTP or as
//! child processes, so it behaves the same on XFCE, GNOME, KDE, sway, or a headless box with a
//! speaker attached.
//!
//! Two behavioural requirements, both from real use rather than theory:
//!
//! * **Do not clip the first word.** Opening an audio sink (especially Bluetooth) loses the first
//!   fraction of a second, so synthesised speech is padded with a short lead-in of silence before
//!   playback. Without it the listener reliably misses the opening word.
//! * **Do not clip the first word of dictation either.** The mic must be opened before the previous
//!   consumer is released; the talk backend handles that, and this module never stops it to start it.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

/// How speech is produced.
#[derive(Clone, Debug, PartialEq)]
pub enum Tts {
    /// POST `{text, voice, speed}` to a Kokoro-style endpoint; the reply is a WAV.
    Kokoro {
        url: String,
        voice: String,
        speed: f64,
    },
    /// Run a command with the text as its single argument.
    Command(String),
    Off,
}

/// Where dictation comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Stt {
    /// A streaming service that accumulates a transcript; read it from `GET <url>/state`.
    Streaming { url: String },
    /// Run a command that prints the transcript on stdout.
    Command(String),
    Off,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceConfig {
    pub tts: Tts,
    pub stt: Stt,
}

impl Default for VoiceConfig {
    /// Defaults are deliberately OFF.
    ///
    /// Pointing at a service that may not exist would make the feature fail in a confusing way on
    /// someone else's machine, and a hard-coded voice is a taste decision that belongs to the user.
    /// Set a Kokoro endpoint or a command in `<data_dir>/voice.json`; the file is written with
    /// these values on first run so the shape is discoverable.
    fn default() -> Self {
        Self { tts: Tts::Off, stt: Stt::Off }
    }
}

impl VoiceConfig {
    /// Load `<data_dir>/voice.json`, writing the defaults on first use so the file is discoverable.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("voice.json");
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                return Self::from_json(&v);
            }
        }
        let cfg = Self::default();
        if fs::create_dir_all(data_dir).is_ok() {
            let _ = fs::write(&path, serde_json::to_string_pretty(&cfg.to_json()).unwrap_or_default());
        }
        cfg
    }

    pub fn to_json(&self) -> Value {
        let tts = match &self.tts {
            Tts::Kokoro { url, voice, speed } => {
                json!({"kind":"kokoro","url":url,"voice":voice,"speed":speed})
            }
            Tts::Command(c) => json!({"kind":"command","command":c}),
            Tts::Off => json!({"kind":"off"}),
        };
        let stt = match &self.stt {
            Stt::Streaming { url } => json!({"kind":"streaming","url":url}),
            Stt::Command(c) => json!({"kind":"command","command":c}),
            Stt::Off => json!({"kind":"off"}),
        };
        json!({"tts": tts, "stt": stt})
    }

    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let tts = match v["tts"]["kind"].as_str().unwrap_or("") {
            "kokoro" => Tts::Kokoro {
                url: v["tts"]["url"].as_str().map(str::to_string)
                    .unwrap_or_else(|| match &d.tts { Tts::Kokoro { url, .. } => url.clone(), _ => String::new() }),
                voice: v["tts"]["voice"].as_str().unwrap_or("").to_string(),
                speed: v["tts"]["speed"].as_f64().unwrap_or(1.0),
            },
            "command" => Tts::Command(v["tts"]["command"].as_str().unwrap_or("").to_string()),
            "off" => Tts::Off,
            _ => d.tts.clone(),
        };
        let stt = match v["stt"]["kind"].as_str().unwrap_or("") {
            "streaming" => Stt::Streaming {
                url: v["stt"]["url"].as_str().unwrap_or("").to_string(),
            },
            "command" => Stt::Command(v["stt"]["command"].as_str().unwrap_or("").to_string()),
            "off" => Stt::Off,
            _ => d.stt.clone(),
        };
        Self { tts, stt }
    }
}

/// Prepend silence to a WAV so the speaker is already open when the first syllable arrives.
///
/// Opening an audio sink costs a few hundred milliseconds on Bluetooth and the beginning of the
/// stream is simply lost. Inserting lead-in frames means the lost part is silence.
pub fn with_lead_in(wav: &[u8], lead_in_ms: u32) -> Option<Vec<u8>> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    // Walk the chunks rather than assuming a canonical 44-byte header.
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32)> = None; // (channels, bits, sample_rate)
    let mut data: Option<(usize, usize)> = None; // (offset, len)
    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let len = u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]) as usize;
        let body = pos + 8;
        if id == b"fmt " && body + 16 <= wav.len() {
            let channels = u16::from_le_bytes([wav[body + 2], wav[body + 3]]);
            let rate = u32::from_le_bytes([
                wav[body + 4], wav[body + 5], wav[body + 6], wav[body + 7],
            ]);
            let bits = u16::from_le_bytes([wav[body + 14], wav[body + 15]]);
            fmt = Some((channels, bits, rate));
        } else if id == b"data" {
            data = Some((body, len.min(wav.len().saturating_sub(body))));
        }
        pos = body + len + (len & 1);
    }
    let (channels, bits, rate) = fmt?;
    let (d_off, d_len) = data?;
    let frame = (bits as usize / 8) * channels as usize;
    if frame == 0 {
        return None;
    }
    let silence_frames = (rate as u64 * lead_in_ms as u64 / 1000) as usize;
    let silence = vec![0u8; silence_frames * frame];
    let new_len = silence.len() + d_len;

    let mut out = Vec::with_capacity(new_len + 64);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + new_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * frame as u32).to_le_bytes());
    out.extend_from_slice(&(frame as u16).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(new_len as u32).to_le_bytes());
    out.extend_from_slice(&silence);
    out.extend_from_slice(&wav[d_off..d_off + d_len]);
    Some(out)
}

/// A blocking HTTP client with a fixed timeout.
///
/// The engine already links reqwest (with the blocking feature) for its providers, so talking to a
/// speech service in-process costs no new dependency and spawns no subprocess per utterance.
fn http(timeout: std::time::Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("could not build HTTP client: {e}"))
}

/// POST to a Kokoro-style endpoint and return the WAV.
pub fn kokoro_fetch(url: &str, voice: &str, speed: f64, text: &str) -> Result<Vec<u8>, String> {
    let client = http(std::time::Duration::from_secs(180))?;
    let resp = client
        .post(url)
        .json(&json!({"text": text, "voice": voice, "speed": speed}))
        .send()
        .map_err(|e| format!("speech endpoint unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("speech endpoint returned HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .map_err(|e| format!("could not read audio: {e}"))?
        .to_vec();
    if bytes.len() < 44 {
        return Err("speech endpoint returned no audio".into());
    }
    Ok(bytes)
}

/// Fetch audio without playing it — used by `quixi --say --dry-run` so the endpoint and the
/// lead-in path can be checked without making noise on someone's machine.
pub fn fetch_for_test(url: &str, voice: &str, speed: f64, text: &str) -> Result<Vec<u8>, String> {
    kokoro_fetch(url, voice, speed, text)
}

/// Speak `text` out of the default sink.
pub fn speak(cfg: &VoiceConfig, text: &str, lead_in_ms: u32) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("nothing to say".into());
    }
    match &cfg.tts {
        Tts::Off => Err("text-to-speech is off".into()),
        Tts::Command(cmd) => {
            let status = Command::new("sh")
                .arg("-c")
                .arg(format!("{cmd} \"$1\""))
                .arg("sh")
                .arg(text)
                .status()
                .map_err(|e| format!("could not run {cmd}: {e}"))?;
            if status.success() { Ok(()) } else { Err(format!("{cmd} exited {status}")) }
        }
        Tts::Kokoro { url, voice, speed } => {
            let wav = kokoro_fetch(url, voice, *speed, text)?;
            let padded = with_lead_in(&wav, lead_in_ms).unwrap_or(wav);
            let path = std::env::temp_dir().join(format!(
                "quixi-speak-{}-{}.wav",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            fs::write(&path, &padded).map_err(|e| format!("could not write audio: {e}"))?;
            let played = Command::new("paplay").arg(&path).status();
            let _ = fs::remove_file(&path);
            match played {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(format!("paplay exited {s}")),
                Err(e) => Err(format!("could not play audio (is pipewire/pulse running?): {e}")),
            }
        }
    }
}

/// Read the accumulated transcript from a streaming talk service.
pub fn transcript(cfg: &VoiceConfig) -> Result<String, String> {
    match &cfg.stt {
        Stt::Off => Err("speech-to-text is off".into()),
        Stt::Command(cmd) => {
            let out = Command::new("sh")
                .args(["-c", cmd])
                .output()
                .map_err(|e| format!("could not run {cmd}: {e}"))?;
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Stt::Streaming { url } => {
            let client = http(std::time::Duration::from_secs(5))?;
            let v: Value = client
                .get(format!("{url}/state"))
                .send()
                .and_then(|r| r.json::<Value>())
                .map_err(|e| format!("could not reach the talk service: {e}"))?;
            Ok(v["transcript"].as_str().unwrap_or("").to_string())
        }
    }
}

/// Health of both directions, for a settings screen that must not lie about availability.
pub fn status(cfg: &VoiceConfig) -> Value {
    let tts_ok = match &cfg.tts {
        Tts::Off => false,
        Tts::Command(_) => true,
        Tts::Kokoro { url, .. } => endpoint_ok(&format!("{url}health")),
    };
    let (stt_ok, engine) = match &cfg.stt {
        Stt::Off => (false, String::new()),
        Stt::Command(_) => (true, "command".into()),
        Stt::Streaming { url } => {
            let v = http(std::time::Duration::from_secs(3))
                .and_then(|c| {
                    c.get(format!("{url}/state"))
                        .send()
                        .and_then(|r| r.json::<Value>())
                        .map_err(|e| e.to_string())
                })
                .unwrap_or(Value::Null);
            (
                v["ok"].as_bool().unwrap_or(false),
                v["engine"].as_str().unwrap_or("unknown").to_string(),
            )
        }
    };
    let tts_backend = match &cfg.tts {
        Tts::Kokoro { .. } => "kokoro",
        Tts::Command(_) => "command",
        Tts::Off => "off",
    };
    json!({
        "tts": {"available": tts_ok, "backend": tts_backend},
        "stt": {"available": stt_ok, "engine": engine},
    })
}

fn endpoint_ok(url: &str) -> bool {
    http(std::time::Duration::from_secs(3))
        .and_then(|c| c.get(url).send().map_err(|e| e.to_string()))
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: &[u8], channels: u16, bits: u16, rate: u32) -> Vec<u8> {
        let frame = (bits as usize / 8) * channels as usize;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&((36 + samples.len()) as u32).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * frame as u32).to_le_bytes());
        v.extend_from_slice(&(frame as u16).to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        v.extend_from_slice(samples);
        v
    }

    #[test]
    fn lead_in_inserts_silence_and_keeps_the_audio() {
        let src = wav(&[1u8; 400], 1, 16, 16000);
        let out = with_lead_in(&src, 250).expect("valid wav");
        // 250 ms at 16 kHz, 16-bit mono = 4000 frames = 8000 bytes
        let data_len = out.len() - 44;
        assert_eq!(data_len, 400 + 8000);
        assert!(out[44..44 + 8000].iter().all(|b| *b == 0), "lead-in must be silent");
        assert_eq!(&out[44 + 8000..], &[1u8; 400], "original samples preserved");
    }

    #[test]
    fn lead_in_respects_stereo_and_bit_depth() {
        let src = wav(&[2u8; 480], 2, 16, 48000);
        let out = with_lead_in(&src, 100).expect("valid wav");
        // 100 ms at 48 kHz stereo 16-bit = 4800 frames * 4 bytes = 19200 bytes
        assert_eq!(out.len() - 44, 480 + 19200);
    }

    #[test]
    fn lead_in_refuses_non_wav_input() {
        assert!(with_lead_in(b"not a wav at all, really not", 100).is_none());
        assert!(with_lead_in(&[], 100).is_none());
    }

    #[test]
    fn config_round_trips_and_keeps_the_defaults() {
        let cfg = VoiceConfig::default();
        let back = VoiceConfig::from_json(&cfg.to_json());
        assert_eq!(cfg, back);
        // Defaults are deliberately off: no endpoint is assumed to exist on this machine.
        assert!(matches!(cfg.tts, Tts::Off));
        assert!(matches!(cfg.stt, Stt::Off));
    }

    #[test]
    fn a_partial_config_keeps_what_was_set_and_defaults_the_rest() {
        let cfg = VoiceConfig::from_json(&json!({"tts": {"kind": "kokoro"}}));
        match cfg.tts {
            Tts::Kokoro { ref voice, speed, .. } => {
                assert_eq!(voice, "");
                assert_eq!(speed, 1.0);
            }
            _ => panic!("an explicit kokoro tts must be kept"),
        }
        assert!(matches!(cfg.stt, Stt::Off));
    }

    #[test]
    fn speaking_nothing_is_an_error_not_a_crash() {
        let cfg = VoiceConfig::default();
        assert!(speak(&cfg, "   ", 250).is_err());
    }

    #[test]
    fn tts_off_reports_clearly() {
        let cfg = VoiceConfig { tts: Tts::Off, stt: Stt::Off };
        assert!(speak(&cfg, "hi", 250).unwrap_err().contains("off"));
        assert!(transcript(&cfg).unwrap_err().contains("off"));
    }
}
