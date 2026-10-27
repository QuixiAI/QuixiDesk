;(function() {
  if (window.__quixi_provider_ui_installed__) return;
  window.__quixi_provider_ui_installed__ = true;

  // Ensure WKWebView always reports visible and cai_hub_preview_optout is enabled so
  // scheduled task starter templates / 'Create with QuixiDesk' never hang on 'Starting…'.
  try {
    window.localStorage.setItem('cai_hub_preview_optout', '1');
    Object.defineProperty(document, 'visibilityState', { get: function() { return 'visible'; }, configurable: true });
    Object.defineProperty(document, 'hidden', { get: function() { return false; }, configurable: true });
  } catch (_) {}

  // Ensure requestAnimationFrame still fires within 16ms if WKWebView is occluded/backgrounded
  // so Radix Dialog/AlertDialog presence transitions never stall when automated or backgrounded.
  try {
    var origRAF = window.requestAnimationFrame.bind(window);
    window.requestAnimationFrame = function(cb) {
      var fired = false;
      var fallbackTimer = window.setTimeout(function() {
        if (!fired) {
          fired = true;
          cb(performance.now());
        }
      }, 32);
      return origRAF(function(ts) {
        if (!fired) {
          fired = true;
          window.clearTimeout(fallbackTimer);
          cb(ts);
        }
      });
    };
  } catch (_) {}

  // Web Animations API watchdog.
  //
  // Some WAAPI transitions never start inside WKWebView when the view was occluded (file
  // preview drawer, right sidebar). The fix must NOT force-finish healthy animations: the
  // renderer's animation-driven components re-trigger on `finish`, which turns into an
  // endless create -> finish -> create loop that pins the WebContent process at 100% CPU
  // and freezes the whole UI (observed via `sample`: WebCore::processIterableKeyframes).
  //
  // So: only nudge animations that never started, and only finish ones that are genuinely
  // stuck at currentTime 0 after a grace period.
  try {
    if (typeof Element !== 'undefined' && Element.prototype && typeof Element.prototype.animate === 'function' && !Element.prototype.animate.__quixi_waapi_fix__) {
      var origAnimate = Element.prototype.animate;
      var patchedAnimate = function(keyframes, options) {
        var anim = origAnimate.apply(this, arguments);
        try {
          if (!anim) return anim;
          if (anim.playState === 'idle') {
            try { anim.play(); } catch (_) {}
          } else {
            window.setTimeout(function() {
              try {
                var atZero = (typeof anim.currentTime === 'number' ? anim.currentTime : 0) === 0;
                if (atZero && (anim.playState === 'running' || anim.playState === 'paused') && typeof anim.finish === 'function') {
                  anim.finish();
                }
              } catch (_) {}
            }, 400);
          }
        } catch (_) {}
        return anim;
      };
      patchedAnimate.__quixi_waapi_fix__ = true;
      Element.prototype.animate = patchedAnimate;
    }
  } catch (_) {}

  var state = null;
  var selectedProviderId = 'openrouter';
  var isOverlayOpen = false;
  var isFirstRunGate = false;
  var providersDirty = false;
  var oauthSession = null;
  var oauthPollTimer = null;
  var statusBanner = null; // { kind: 'ok' | 'err' | 'info', text: string }

  // Clear stale React Query IndexedDB cache & stale claude-sonnet-4-6 localStorage entries,
  // and lock the unified workspace surface to full-featured ('cowork') mode so folder picking,
  // file permissions, and steps/context are always exposed without mode toggles.
  try {
    if (window.indexedDB && typeof window.indexedDB.deleteDatabase === 'function') {
      window.indexedDB.deleteDatabase('keyval-store');
    }
    if (window.localStorage) {
      var origGetItem = Storage.prototype.getItem;
      var origSetItem = Storage.prototype.setItem;
      Storage.prototype.getItem = function(key) {
        if (this === window.localStorage && typeof key === 'string') {
          if (key === 'chorus-composer-surface-kind' || key.indexOf('chorus-composer-surface-kind:') === 0) {
            return 'cowork';
          }
          if (key === 'dframe-store') {
            var raw = origGetItem.call(this, key);
            if (raw) {
              try {
                var parsed = JSON.parse(raw);
                if (parsed && parsed.state && parsed.state.lastKnownMode === 'code') {
                  parsed.state.lastKnownMode = 'cowork';
                  return JSON.stringify(parsed);
                }
              } catch (_) {}
            }
          }
        }
        return origGetItem.call(this, key);
      };
      Storage.prototype.setItem = function(key, val) {
        if (this === window.localStorage && typeof key === 'string') {
          if (key === 'chorus-composer-surface-kind' || key.indexOf('chorus-composer-surface-kind:') === 0) {
            return origSetItem.call(this, key, 'cowork');
          }
        }
        return origSetItem.call(this, key, val);
      };
      origSetItem.call(window.localStorage, 'chorus-composer-surface-kind', 'cowork');
      for (var lsIdx = window.localStorage.length - 1; lsIdx >= 0; lsIdx--) {
        var k = window.localStorage.key(lsIdx);
        if (k) {
          if (k.indexOf('chorus-composer-surface-kind:') === 0) {
            origSetItem.call(window.localStorage, k, 'cowork');
            continue;
          }
          var v = origGetItem.call(window.localStorage, k) || '';
          if (v.indexOf('claude-sonnet-4-6') !== -1 || v.indexOf('claude-opus-4-7') !== -1) {
            window.localStorage.removeItem(k);
          }
        }
      }
    }
  } catch (_) {}

  // Canonical Quixi `q.` SVG App Icon (dark squircle + teal geometric `q` + violet `.`)
  var QUIXI_APP_ICON_SVG =
    '<svg viewBox="0 0 100 100" width="32" height="32" fill="none" xmlns="http://www.w3.org/2000/svg">' +
      '<rect width="100" height="100" rx="22" fill="#0B0D13"/>' +
      '<rect x="1.5" y="1.5" width="97" height="97" rx="20.5" stroke="#262B38" stroke-width="2"/>' +
      '<path fill="#18C9B0" fill-rule="evenodd" clip-rule="evenodd" d="M 48.5 25 C 48.5 21.96 50.96 19.5 54 19.5 C 57.04 19.5 59.5 21.96 59.5 25 L 59.5 79.5 C 59.5 82.54 57.04 85 54 85 C 50.96 85 48.5 82.54 48.5 79.5 L 48.5 60.55 A 22.5 22.5 0 1 1 48.5 23.45 Z M 37.5 31 A 11 11 0 1 0 37.5 53 A 11 11 0 1 0 37.5 31 Z"/>' +
      '<circle cx="76.5" cy="74" r="9.5" fill="#818CF8"/>' +
    '</svg>';

  // Transparent-background two-tone Quixi `q.` mark SVG
  var QUIXI_MARK_INNER_PATHS =
    '<path fill="#18C9B0" fill-rule="evenodd" clip-rule="evenodd" d="M 48.5 25 C 48.5 21.96 50.96 19.5 54 19.5 C 57.04 19.5 59.5 21.96 59.5 25 L 59.5 79.5 C 59.5 82.54 57.04 85 54 85 C 50.96 85 48.5 82.54 48.5 79.5 L 48.5 60.55 A 22.5 22.5 0 1 1 48.5 23.45 Z M 37.5 31 A 11 11 0 1 0 37.5 53 A 11 11 0 1 0 37.5 31 Z"/>' +
    '<circle cx="76.5" cy="74" r="9.5" fill="#818CF8"/>';

  var QUIXI_MARK_SVG =
    '<svg viewBox="0 0 100 100" width="16" height="16" fill="none" xmlns="http://www.w3.org/2000/svg" data-quixi-logo="1">' +
      QUIXI_MARK_INNER_PATHS +
    '</svg>';

  // Provider-specific SVG logo icons (shown ONLY inside the Provider Selection Screen `#quixi-provider-overlay`)
  function getProviderLogoSvg(providerId, size) {
    var sz = size || 24;
    switch (providerId) {
      case 'openrouter':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="openrouter" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#4F46E5"/>' +
            '<path d="M5 12h5l3-5h6M16 4l3 3-3 3M10 12l3 5h6M16 14l3 3-3 3" stroke="#FFFFFF" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>' +
          '</svg>'
        );
      case 'custom':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="custom" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#0EA5E9"/>' +
            '<path d="M8 9l-3 3 3 3M16 9l3 3-3 3M13.5 7.5l-3 9" stroke="#FFFFFF" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>' +
          '</svg>'
        );
      case 'openai':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="openai" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#10A37F"/>' +
            '<path d="M12 5.2a3.8 3.8 0 0 1 3.55 2.45 3.8 3.8 0 0 1 2.75 5.1A3.8 3.8 0 0 1 15 18.2a3.8 3.8 0 0 1-6.55-.45 3.8 3.8 0 0 1-2.75-5.1A3.8 3.8 0 0 1 9 6.8c.7-.98 1.82-1.6 3-1.6Z" stroke="#FFFFFF" stroke-width="1.6"/>' +
            '<circle cx="12" cy="12" r="2.2" fill="#FFFFFF"/>' +
          '</svg>'
        );
      case 'xai':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="xai" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#141413"/>' +
            '<path d="M6.5 6.5L17.5 17.5M17.5 6.5L6.5 17.5" stroke="#FFFFFF" stroke-width="2.3" stroke-linecap="round"/>' +
          '</svg>'
        );
      case 'anthropic':
        // Anthropic / Claude logo icon allowed ONLY in the Provider Selection Screen
        return (
          '<svg class="qp-prov-svg" data-provider-logo="anthropic" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#D97757"/>' +
            '<path d="M14.2 6.5h2.2l3.6 11h-2.2l-.75-2.4h-3.7l-.75 2.4H10.4l3.8-11Zm.45 6.6h2.4L15.85 9l-1.2 4.1ZM8.4 6.5h2.25l-3.8 11H4.6l3.8-11Z" fill="#FFFFFF"/>' +
          '</svg>'
        );
      case 'google':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="google" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#1A73E8"/>' +
            '<path d="M12 4.5C12.7 8.6 15.4 11.3 19.5 12C15.4 12.7 12.7 15.4 12 19.5C11.3 15.4 8.6 12.7 4.5 12C8.6 11.3 11.3 8.6 12 4.5Z" fill="#FFFFFF"/>' +
          '</svg>'
        );
      case 'deepseek':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="deepseek" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#2563EB"/>' +
            '<path d="M5.5 13.5c1.5-4 5-6 9-5.5 2 .25 3.5 1.5 4 3.5-1.5 3.5-5 5.5-9 5-2.2-.3-3.5-1.5-4-3Z" stroke="#FFFFFF" stroke-width="1.8" fill="none"/>' +
            '<circle cx="14.5" cy="11.5" r="1.3" fill="#FFFFFF"/>' +
          '</svg>'
        );
      case 'groq':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="groq" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#F55036"/>' +
            '<path d="M13 5L7.5 13h4.2l-.7 6 5.5-8h-4.2L13 5Z" fill="#FFFFFF"/>' +
          '</svg>'
        );
      case 'together':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="together" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#0F6FFF"/>' +
            '<circle cx="9.5" cy="12" r="3.5" stroke="#FFFFFF" stroke-width="1.8"/>' +
            '<circle cx="14.5" cy="12" r="3.5" stroke="#FFFFFF" stroke-width="1.8"/>' +
          '</svg>'
        );
      case 'fireworks':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="fireworks" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#7C3AED"/>' +
            '<path d="M12 5v14M5 12h14M7.2 7.2l9.6 9.6M16.8 7.2l-9.6 9.6" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round"/>' +
          '</svg>'
        );
      case 'mistral':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="mistral" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#FA520F"/>' +
            '<path d="M6 17V7h2.5l3.5 4.5L15.5 7H18v10h-2.5v-5.5L12 16l-3.5-4.5V17H6Z" fill="#FFFFFF"/>' +
          '</svg>'
        );
      case 'cohere':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="cohere" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#39594D"/>' +
            '<circle cx="9" cy="10" r="2.8" fill="#D18EE2"/>' +
            '<circle cx="15" cy="10" r="2.8" fill="#FFFFFF"/>' +
            '<circle cx="12" cy="15.2" r="2.8" fill="#FF7759"/>' +
          '</svg>'
        );
      case 'perplexity':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="perplexity" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#20808D"/>' +
            '<path d="M12 5v14M6.5 8.5l11 7M17.5 8.5l-11 7" stroke="#FFFFFF" stroke-width="1.8" stroke-linecap="round"/>' +
          '</svg>'
        );
      case 'venice':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="venice" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#DC2626"/>' +
            '<path d="M6.5 7.5L12 17.5l5.5-10" stroke="#FFFFFF" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>' +
          '</svg>'
        );
      case 'nvidia':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="nvidia" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#76B900"/>' +
            '<path d="M5.5 12c2.5-3.8 6-4.5 9.5-2.5 2 1.2 3 3 3.5 3.5-2 2.8-5.5 4-9 2.5-2-.8-3.2-2.2-4-3.5Z" stroke="#FFFFFF" stroke-width="1.8"/>' +
            '<circle cx="12" cy="12" r="1.8" fill="#FFFFFF"/>' +
          '</svg>'
        );
      case 'bedrock':
        return (
          '<svg class="qp-prov-svg" data-provider-logo="bedrock" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#FF9900"/>' +
            '<path d="M12 5.5l6 3.4v6.2l-6 3.4-6-3.4V8.9l6-3.4Z" stroke="#FFFFFF" stroke-width="1.8" stroke-linejoin="round"/>' +
            '<path d="M6 8.9l6 3.4 6-3.4M12 12.3v6.2" stroke="#FFFFFF" stroke-width="1.6"/>' +
          '</svg>'
        );
      case 'ollama':
      case 'lmstudio':
      case 'vllm':
      default:
        return (
          '<svg class="qp-prov-svg" data-provider-logo="' + escapeHtml(providerId) + '" width="' + sz + '" height="' + sz + '" viewBox="0 0 24 24" fill="none">' +
            '<rect width="24" height="24" rx="6" fill="#3B82F6"/>' +
            '<rect x="6" y="6.5" width="12" height="5" rx="1.5" stroke="#FFFFFF" stroke-width="1.7"/>' +
            '<rect x="6" y="12.5" width="12" height="5" rx="1.5" stroke="#FFFFFF" stroke-width="1.7"/>' +
            '<circle cx="8.8" cy="9" r="0.9" fill="#FFFFFF"/>' +
            '<circle cx="8.8" cy="15" r="0.9" fill="#FFFFFF"/>' +
          '</svg>'
        );
    }
  }

  function startLogoGuard() {
    function scrubBrandingAndMascots() {
      try {
        if (typeof dockHeaderPills === 'function') {
          dockHeaderPills();
        }

        // Finish any stalled finite WAAPI animations (file preview drawer, right sidebar transitions)
        // and cancel width-locking morph animations inside [data-cds="TurnStatus"]
        if (typeof document.getAnimations === 'function') {
          var anims = document.getAnimations();
          for (var aIdx = 0; aIdx < anims.length; aIdx++) {
            try {
              var a = anims[aIdx];
              var target = a && a.effect && a.effect.target;
              if (target && target.closest && target.closest('[data-cds="TurnStatus"]')) {
                if (typeof a.cancel === 'function') a.cancel();
                continue;
              }
              var timing = a && a.effect && typeof a.effect.getTiming === 'function' ? a.effect.getTiming() : null;
              if (!timing || timing.iterations !== Infinity) {
                if (a && typeof a.finish === 'function' && a.playState === 'running') {
                  a.finish();
                }
              }
            } catch (_) {}
          }
        }

        // Hide empty unlocalized Settings nav rows (reflect/time-limits) and legacy QuixiDesk Code row
        var settingsNavBtns = document.querySelectorAll('[role="dialog"] nav button.hit-row');
        for (var nbIdx = 0; nbIdx < settingsNavBtns.length; nbIdx++) {
          var nb = settingsNavBtns[nbIdx];
          var labelSpan = nb.querySelector('span.truncate');
          var lblTxt = labelSpan ? (labelSpan.textContent || '').trim() : (nb.textContent || '').trim();
          var li = nb.closest('li') || nb;
          if (!lblTxt || lblTxt === 'QuixiDesk Code') {
            li.style.display = 'none';
          }
        }

        // Hide Code filter pill and New Code session option in Search (⌘K) command palette
        var cmdPaletteInput = document.getElementById('command-palette-input');
        var cmdPalette = document.getElementById('command-palette') || (cmdPaletteInput && cmdPaletteInput.closest('[role="dialog"]'));
        if (cmdPalette) {
          var cpEls = cmdPalette.querySelectorAll('button, [role="option"]');
          for (var cpIdx = 0; cpIdx < cpEls.length; cpIdx++) {
            var cpEl = cpEls[cpIdx];
            var cpTxt = (cpEl.textContent || '').trim();
            if (cpTxt === 'Code' || cpTxt.indexOf('New Code session') !== -1) {
              cpEl.style.display = 'none';
            }
          }
        }

        // Redirect legacy /code or /epitaxy routes to the unified /new workspace
        if (location.pathname === '/code' || location.pathname.indexOf('/code/') === 0 || location.pathname === '/epitaxy') {
          history.replaceState(null, '', '/new');
          window.dispatchEvent(new PopStateEvent('popstate'));
        }

        // Ensure the composer surface is always locked to full-featured ('cowork') mode
        // so optional folder picking, file permissions, and steps/context are always active.
        var coworkSeg = document.querySelector('[data-surface-segment="cowork"]');
        if (coworkSeg) {
          var coworkRadio = coworkSeg.closest('[role="radio"]');
          if (coworkRadio && !coworkRadio.hasAttribute('data-checked') && coworkRadio.getAttribute('aria-checked') !== 'true') {
            coworkRadio.click();
          }
        }

        // Set high-contrast Dark Obsidian mode on the left navigation dock (.dframe-sidebar)
        var sidebars = document.querySelectorAll('.dframe-sidebar');
        for (var sIdx = 0; sIdx < sidebars.length; sIdx++) {
          if (sidebars[sIdx].getAttribute('data-mode') !== 'dark') {
            sidebars[sIdx].setAttribute('data-mode', 'dark');
          }
        }

        // 1. Replace any remaining 96x96 Clawd pixel mascot SVG or Spark spinner image with the static Quixi `q.` icon
        var clawdPaths = document.querySelectorAll('svg path[d^="M14.3477"], svg[data-cds="Spark"] image');
        for (var i = 0; i < clawdPaths.length; i++) {
          var svg = clawdPaths[i].closest('svg');
          if (svg && !svg.getAttribute('data-quixi-logo')) {
            svg.setAttribute('data-quixi-logo', '1');
            svg.setAttribute('viewBox', '0 0 100 100');
            svg.innerHTML =
              '<rect width="100" height="100" rx="22" fill="#0B0D13"/>' +
              '<rect x="1.5" y="1.5" width="97" height="97" rx="20.5" stroke="#262B38" stroke-width="2" fill="none"/>' +
              QUIXI_MARK_INNER_PATHS;
          }
        }

        // 2. Scrub any remaining "Anthropic" or "Claude" or "You're here!" text nodes outside the provider config overlay
        var walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT, null, false);
        var node;
        while ((node = walker.nextNode())) {
          var val = node.nodeValue;
          if (
            !val ||
            (val.indexOf('Anthropic') === -1 &&
             val.indexOf('Claude in Chrome') === -1 &&
             val.indexOf('Claude Browser') === -1 &&
             val.indexOf('Claude Light') === -1 &&
             val.indexOf('Claude Dark') === -1 &&
             val.indexOf('You\u2019re here!') === -1 &&
             val.indexOf("You're here!") === -1)
          ) continue;
          var parent = node.parentElement;
          if (parent && parent.closest && parent.closest('#quixi-provider-overlay')) continue;
          var next = val
            .replace(/You[\u2019']re here!/g, 'QuixiDesk Studio \u2014 Ready to Build')
            .replace(/Anthropic[\u2019']s guidelines/g, 'workspace guidelines')
            .replace(/Anthropic Serif/g, 'Quixi Sans')
            .replace(/Claude in Chrome/g, 'Quixi in Chrome')
            .replace(/Claude Browser/g, 'Quixi Browser')
            .replace(/Claude Light/g, 'Quixi Cyber-Slate Light')
            .replace(/Claude Dark/g, 'Quixi Obsidian Dark')
            .replace(/Anthropic/g, 'Quixi');
          if (next !== val) {
            node.nodeValue = next;
          }
        }
      } catch (_) {}
    }

    scrubBrandingAndMascots();
    var scrubTimer = null;
    var obs = new MutationObserver(function() {
      if (scrubTimer) return;
      scrubTimer = window.setTimeout(function() {
        scrubTimer = null;
        scrubBrandingAndMascots();
      }, 25);
    });
    if (document.documentElement) {
      obs.observe(document.documentElement, { childList: true, subtree: true });
    }
  }

  function api(path, method, body) {
    var opts = {
      method: method || 'GET',
      headers: { 'Content-Type': 'application/json' }
    };
    if (body !== undefined) {
      opts.body = JSON.stringify(body);
    }
    return fetch('app://localhost' + path, opts).then(function(r) { return r.json(); });
  }

  function injectStyles() {
    if (document.getElementById('quixi-provider-ui-styles')) return;
    var style = document.createElement('style');
    style.id = 'quixi-provider-ui-styles';
    style.textContent = [
      '/* ========================================================================== */',
      '/* QUIXIDESK CYBER-SLATE & ELECTRIC TEAL-INDIGO DESIGN SYSTEM                 */',
      '/* ========================================================================== */',
      ':root, html.cds-root, [data-theme="claude"], [data-theme="claude"][data-mode="light"], [data-theme="claude"][data-mode="dark"], [data-color-version], [data-color-version="v2"][data-theme="claude"] {',
      '  /* 30-Stop Static Cyber-Slate Gray Scale (0 = White -> 900 = Deepest Obsidian) */',
      '  /* CDS automatically maps light & dark surfaces/text from these 30 stops!      */',
      '  --cds-white: #FFFFFF !important;',
      '  --cds-gray-0: #FFFFFF !important;',
      '  --cds-gray-10: #F8FAFC !important;',
      '  --cds-gray-20: #F4F7FB !important;',
      '  --cds-gray-30: #F1F5F9 !important;',
      '  --cds-gray-40: #ECF2F8 !important;',
      '  --cds-gray-50: #E8EEF6 !important;',
      '  --cds-gray-60: #E2E8F0 !important;',
      '  --cds-gray-70: #DCE4EE !important;',
      '  --cds-gray-80: #D5DFEC !important;',
      '  --cds-gray-90: #CEDAE9 !important;',
      '  --cds-gray-100: #CBD5E1 !important;',
      '  --cds-gray-150: #B8C6D8 !important;',
      '  --cds-gray-200: #94A3B8 !important;',
      '  --cds-gray-250: #7E90A8 !important;',
      '  --cds-gray-300: #64748B !important;',
      '  --cds-gray-350: #56667C !important;',
      '  --cds-gray-400: #475569 !important;',
      '  --cds-gray-450: #3D4B5F !important;',
      '  --cds-gray-500: #334155 !important;',
      '  --cds-gray-550: #2C3A4E !important;',
      '  --cds-gray-600: #263449 !important;',
      '  --cds-gray-650: #212E42 !important;',
      '  --cds-gray-700: #1E293B !important;',
      '  --cds-gray-750: #192335 !important;',
      '  --cds-gray-800: #151F32 !important;',
      '  --cds-gray-810: #141D30 !important;',
      '  --cds-gray-820: #131C2E !important;',
      '  --cds-gray-830: #111A2B !important;',
      '  --cds-gray-840: #0F172A !important;',
      '  --cds-gray-850: #0D1322 !important;',
      '  --cds-gray-860: #0C1120 !important;',
      '  --cds-gray-870: #0B101E !important;',
      '  --cds-gray-880: #0A0E1B !important;',
      '  --cds-gray-890: #090D18 !important;',
      '  --cds-gray-900: #070B14 !important;',
      '  --cds-hsl-white: 0 0% 100% !important;',
      '  --cds-hsl-gray-0: 0 0% 100% !important;',
      '  --cds-hsl-gray-10: 210 40% 98% !important;',
      '  --cds-hsl-gray-20: 214 45% 97% !important;',
      '  --cds-hsl-gray-30: 210 40% 96.1% !important;',
      '  --cds-hsl-gray-40: 210 43% 95% !important;',
      '  --cds-hsl-gray-50: 214 38% 93.7% !important;',
      '  --cds-hsl-gray-60: 214 32% 91.4% !important;',
      '  --cds-hsl-gray-70: 213 32% 90% !important;',
      '  --cds-hsl-gray-80: 214 33% 88% !important;',
      '  --cds-hsl-gray-90: 213 35% 86% !important;',
      '  --cds-hsl-gray-100: 213 27% 84% !important;',
      '  --cds-hsl-gray-150: 214 27% 78% !important;',
      '  --cds-hsl-gray-200: 215 20% 65% !important;',
      '  --cds-hsl-gray-250: 214 19% 58% !important;',
      '  --cds-hsl-gray-300: 215 16% 47% !important;',
      '  --cds-hsl-gray-350: 215 18% 41% !important;',
      '  --cds-hsl-gray-400: 215 19% 35% !important;',
      '  --cds-hsl-gray-450: 215 22% 31% !important;',
      '  --cds-hsl-gray-500: 215 25% 27% !important;',
      '  --cds-hsl-gray-550: 215 28% 24% !important;',
      '  --cds-hsl-gray-600: 216 32% 22% !important;',
      '  --cds-hsl-gray-650: 216 33% 19% !important;',
      '  --cds-hsl-gray-700: 217 33% 17% !important;',
      '  --cds-hsl-gray-750: 219 36% 15% !important;',
      '  --cds-hsl-gray-800: 219 41% 14% !important;',
      '  --cds-hsl-gray-810: 221 41% 13.5% !important;',
      '  --cds-hsl-gray-820: 220 41% 13% !important;',
      '  --cds-hsl-gray-830: 220 43% 12% !important;',
      '  --cds-hsl-gray-840: 222 47% 11% !important;',
      '  --cds-hsl-gray-850: 223 45% 9.2% !important;',
      '  --cds-hsl-gray-860: 225 45% 8.6% !important;',
      '  --cds-hsl-gray-870: 224 46% 8% !important;',
      '  --cds-hsl-gray-880: 226 46% 7.3% !important;',
      '  --cds-hsl-gray-890: 224 45% 6.5% !important;',
      '  --cds-hsl-gray-900: 222 48% 5.3% !important;',
      '  /* Electric Teal & Indigo Brand Tokens (replaces terracotta/clay #D97757) */',
      '  --cds-clay: #14B8A6 !important;',
      '  --cds-clay-emphasized: #0D9488 !important;',
      '  --cds-hsl-clay: 173 80% 40% !important;',
      '  --cds-hsl-clay-emphasized: 175 84% 32% !important;',
      '  --_brand-clay: 173 80% 40% !important;',
      '  --_brand-clay-emphasized: 175 84% 32% !important;',
      '  --accent-brand: 173 80% 40% !important;',
      '  --brand-000: 175 84% 32% !important;',
      '  --brand-100: 173 80% 40% !important;',
      '  --brand-200: 172 66% 50% !important;',
      '  --cds-fill-brand: #0D9488 !important;',
      '  --cds-fill-brand-hover: #14B8A6 !important;',
      '  --cds-fill-brand-secondary: rgba(20, 184, 166, 0.14) !important;',
      '  --cds-text-brand: #0D9488 !important;',
      '  --cds-border-brand: #14B8A6 !important;',
      '  /* Clean Geometric Sans-Serif Typography (replaces Copernicus/Tiempos serif) */',
      '  --font-anthropic-serif: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Inter", system-ui, sans-serif !important;',
      '  --cds-font-prose: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", system-ui, sans-serif !important;',
      '  --cds-font-display: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Inter", system-ui, sans-serif !important;',
      '  --cds-font-voice: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Inter", system-ui, sans-serif !important;',
      '}',
      '[data-mode="light"], :root:not([data-mode="dark"]) {',
      '  --bg-000: 0 0% 100% !important;',
      '  --bg-100: 210 40% 96.1% !important;',
      '  --bg-200: 214 38% 93.5% !important;',
      '  --bg-300: 214 32% 90% !important;',
      '  --bg-400: 213 27% 84% !important;',
      '  --text-000: 222 47% 11% !important;',
      '  --text-100: 222 47% 11% !important;',
      '  --text-200: 215 25% 27% !important;',
      '  --text-300: 215 16% 45% !important;',
      '  --text-400: 215 16% 47% !important;',
      '  --border-100: 215 25% 27% !important;',
      '  --border-200: 215 25% 27% !important;',
      '  --border-300: 215 25% 27% !important;',
      '  --cds-surface-page: #F1F5F9 !important;',
      '  --cds-surface-primary: #FFFFFF !important;',
      '  --cds-surface-secondary: #E8EEF6 !important;',
      '  --cds-surface-tertiary: #E2E8F0 !important;',
      '}',
      '.dframe-root {',
      '  --df-z0: 0 0% 100% !important;',
      '  --df-z1: 210 40% 96.1% !important;',
      '  --df-z2: 214 32% 91.4% !important;',
      '  --df-z3: 213 27% 84% !important;',
      '  --df-z4: 215 20% 65% !important;',
      '  --df-z5: 215 16% 47% !important;',
      '  --df-z6: 215 25% 27% !important;',
      '  --df-bg-page-hsl: 210 40% 96.1% !important;',
      '  --df-bg-page: #F1F5F9 !important;',
      '}',
      '.dframe-root .dframe-card {',
      '  --df-sidebar-bg: #FFFFFF !important;',
      '  background-color: #FFFFFF !important;',
      '  border: 1px solid #CBD5E1 !important;',
      '  border-radius: 12px !important;',
      '  box-shadow: 0 4px 16px rgba(15, 23, 42, 0.05) !important;',
      '}',
      '.font-claude-response, .font-display, .font-serif, [data-cds="Prose"] {',
      '  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", system-ui, sans-serif !important;',
      '  letter-spacing: -0.011em;',
      '}',
      '/* Inline code pills: Indigo-Slate instead of terracotta clay (excluding file viewer code lines) */',
      'code:not(pre code):not(.font-mono-ui):not([class*="break-all"]) {',
      '  background: rgba(79, 70, 229, 0.09) !important;',
      '  color: #4338CA !important;',
      '  border: 1px solid rgba(79, 70, 229, 0.22) !important;',
      '  border-radius: 6px !important;',
      '  padding: 1.5px 6px !important;',
      '  font-weight: 550 !important;',
      '}',
      '[data-mode="dark"] code:not(pre code):not(.font-mono-ui):not([class*="break-all"]) {',
      '  background: rgba(99, 102, 241, 0.16) !important;',
      '  color: #A5B4FC !important;',
      '  border: 1px solid rgba(99, 102, 241, 0.32) !important;',
      '}',
      '/* ========================================================================== */',
      '/* CUSTOM ANIMATED GIF SPINNER (/assets/quixi-spinner.gif) ONLY WHEN ACTIVE   */',
      '/* ========================================================================== */',
      '[data-cds="Spinner"], [data-testid="chat-footer-spark"] > span[data-activity]:not([data-activity="idle"]) {',
      '  background: url("/assets/quixi-spinner.gif") center / contain no-repeat !important;',
      '  border: none !important;',
      '  box-shadow: none !important;',
      '  min-width: 20px !important;',
      '  min-height: 20px !important;',
      '}',
      '[data-testid="chat-footer-spark"] > span[data-activity="idle"] {',
      '  display: none !important;',
      '  background: none !important;',
      '}',
      '[data-cds="Spinner"] > span, [data-cds="Spinner"] > svg, [data-testid="chat-footer-spark"] > span > * {',
      '  display: none !important;',
      '}',
      '/* Prevent WKWebView from stalling user message enter animations at opacity:0 */',
      '.cds-um-enter, .code-user-bubble-enter, .message-fade-up, .placement-fade-in, .placement-fade-in-fast, .placement-rise-in, .task-grid-mount-reveal, [data-cds="UserMessage"] {',
      '  animation: none !important;',
      '  opacity: 1 !important;',
      '  transform: none !important;',
      '}',
      '/* Stacked Home Hero with Static Quixi q. App Icon */',
      '[data-new-page-greeting-spark] {',
      '  position: static !important;',
      '  display: block !important;',
      '  width: 56px !important;',
      '  height: 56px !important;',
      '  margin: 0 auto 10px auto !important;',
      '  transform: none !important;',
      '  filter: drop-shadow(0 8px 22px rgba(20, 184, 166, 0.28)) !important;',
      '}',
      '[data-new-page-greeting-spark] svg {',
      '  transform: none !important;',
      '  width: 100% !important;',
      '  height: 100% !important;',
      '}',
      '*:has(> [data-new-page-greeting-spark]) {',
      '  display: flex !important;',
      '  flex-direction: column !important;',
      '  align-items: center !important;',
      '  justify-content: center !important;',
      '  gap: 6px !important;',
      '  text-align: center !important;',
      '}',
      '[data-new-page-greeting-text], *:has(> [data-new-page-greeting-spark]) h1 {',
      '  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Display", system-ui, sans-serif !important;',
      '  font-weight: 760 !important;',
      '  letter-spacing: -0.028em !important;',
      '  background: linear-gradient(135deg, #0F172A 0%, #0D9488 52%, #4F46E5 100%) !important;',
      '  -webkit-background-clip: text !important;',
      '  -webkit-text-fill-color: transparent !important;',
      '}',
      '[data-mode="dark"] [data-new-page-greeting-text], [data-mode="dark"] *:has(> [data-new-page-greeting-spark]) h1 {',
      '  background: linear-gradient(135deg, #F8FAFC 0%, #2DD4BF 52%, #818CF8 100%) !important;',
      '  -webkit-background-clip: text !important;',
      '  -webkit-text-fill-color: transparent !important;',
      '}',
      '/* ========================================================================== */',
      '/* DISTINCTIVE QUIXIDESK LAYOUT ARCHITECTURE                                  */',
      '/* ========================================================================== */',
      '/* 1. High-Contrast Dark Obsidian Dock Sidebar */',
      '.dframe-sidebar {',
      '  --text-000: 210 40% 98% !important;',
      '  --text-100: 210 40% 96% !important;',
      '  --text-200: 214 32% 88% !important;',
      '  --text-300: 215 25% 76% !important;',
      '  --text-400: 215 20% 68% !important;',
      '  --bg-000: 222 47% 11% !important;',
      '  --bg-100: 222 47% 9% !important;',
      '  --bg-200: 217 33% 16% !important;',
      '  --bg-300: 215 25% 23% !important;',
      '  --bg-400: 215 20% 30% !important;',
      '  --border-100: 215 25% 65% !important;',
      '  --border-200: 215 25% 65% !important;',
      '  --border-300: 215 25% 65% !important;',
      '  --cds-text-primary: #F8FAFC !important;',
      '  --cds-text-secondary: #E2E8F0 !important;',
      '  --cds-text-tertiary: #94A3B8 !important;',
      '  --cds-text-muted: #94A3B8 !important;',
      '  --cds-icon-primary: #F8FAFC !important;',
      '  --cds-icon-secondary: #CBD5E1 !important;',
      '  --cds-icon-tertiary: #94A3B8 !important;',
      '  --cds-surface-primary: #0F172A !important;',
      '  --cds-surface-secondary: #1E293B !important;',
      '  --cds-surface-tertiary: #334155 !important;',
      '  --cds-surface-popover: #1E293B !important;',
      '  --cds-segmented-control-track: rgba(255, 255, 255, 0.09) !important;',
      '  --cds-segmented-control-thumb: #0D9488 !important;',
      '  color: #E2E8F0 !important;',
      '}',
      '.dframe-sidebar:not(.dframe-root[data-collapsed] .dframe-sidebar) {',
      '  background: linear-gradient(180deg, #0B1120 0%, #0F172A 62%, #131C31 100%) !important;',
      '  border-right: 1px solid #1E293B !important;',
      '  border-radius: 0 !important;',
      '  box-shadow: 8px 0 30px rgba(6, 10, 18, 0.16) !important;',
      '}',
      '/* Hide Mode Toggles (Home/Code & Chat/Cowork) and expose features directly inline */',
      '.dframe-sidebar .df-pills, .dframe-sidebar [data-chorus-callout="pills"], .dframe-sidebar [data-compact-mode-switcher], [data-chorus-callout="pills"], [data-compact-mode-switcher], [data-cds="SegmentedControl"][aria-label="Mode"], [data-cds="SegmentedControl"][aria-label="Composer mode"] {',
      '  display: none !important;',
      '}',
      '[data-cds="SegmentedControl"][aria-label="Surface"], [data-cds="SegmentedControl"]:has([data-surface-segment]), div.contents:has(> [data-cds="SegmentedControl"][aria-label="Surface"]), a[aria-label*="Learn more about Cowork"] {',
      '  display: none !important;',
      '}',
      '/* Prevent TurnStatus summary labels from clipping via WAAPI morph width or row-cut masks */',
      '[data-cds="TurnStatus"] [id$="-label"], [data-cds="TurnStatus"] [data-morph-key], [data-cds="TurnStatus"] [data-cds-row-cut] {',
      '  width: auto !important; max-width: 100% !important; mask-image: none !important; -webkit-mask-image: none !important;',
      '}',
      '.dframe-sidebar [data-row] {',
      '  border-radius: 9px !important;',
      '  color: #CBD5E1 !important;',
      '  transition: all 0.14s ease !important;',
      '}',
      '.dframe-sidebar [data-row]:hover {',
      '  background: rgba(255, 255, 255, 0.06) !important;',
      '  color: #F8FAFC !important;',
      '}',
      '.dframe-sidebar [data-row][data-selected] {',
      '  background: linear-gradient(90deg, rgba(20, 184, 166, 0.24) 0%, rgba(79, 70, 229, 0.12) 100%) !important;',
      '  box-shadow: inset 3.5px 0 0 #14B8A6 !important;',
      '  color: #FFFFFF !important;',
      '}',
      '/* 2. Cool Ice-Slate Main Workspace Canvas & Frosted Header Bar */',
      '.dframe-main, .dframe-content {',
      '  background: radial-gradient(circle at 50% 0%, rgba(20, 184, 166, 0.08) 0%, rgba(79, 70, 229, 0.05) 36%, transparent 70%), #F1F5F9 !important;',
      '}',
      '.dframe-header {',
      '  background: rgba(241, 245, 249, 0.84) !important;',
      '  backdrop-filter: blur(12px) !important;',
      '  border-bottom: 1px solid rgba(203, 213, 225, 0.65) !important;',
      '}',
      '[data-cds="Page"] > .sticky.top-0.bg-page {',
      '  background: transparent !important;',
      '  backdrop-filter: blur(12px) !important;',
      '}',
      '/* 3. Architectural Command Deck Composer with Top Teal-Indigo Accent Bar */',
      '[data-composer-card-holder] > div {',
      '  border-radius: 14px !important;',
      '  border: 1.5px solid #CBD5E1 !important;',
      '  background: #FFFFFF !important;',
      '  box-shadow: inset 0 3px 0 0 #14B8A6, 0 14px 34px -8px rgba(15, 23, 42, 0.11) !important;',
      '}',
      '[data-composer-card-holder] > div:focus-within {',
      '  border-color: #14B8A6 !important;',
      '  box-shadow: inset 0 3px 0 0 #4F46E5, 0 0 0 3px rgba(20, 184, 166, 0.16), 0 16px 38px -8px rgba(15, 23, 42, 0.14) !important;',
      '}',
      '/* Gradient Send / Primary CTA Buttons (replaces coral circle & stark white pills) */',
      '[data-composer-card-holder] button.bg-fill-brand, [data-composer-card-holder] button[aria-label*="Send"], [data-composer-card-holder] button[aria-label*="send"], button[data-testid="chat-input-send"], button.bg-fill-brand {',
      '  background: linear-gradient(135deg, #0D9488 0%, #4F46E5 100%) !important;',
      '  color: #FFFFFF !important;',
      '  border-radius: 10px !important;',
      '  border: 1px solid rgba(255, 255, 255, 0.18) !important;',
      '  box-shadow: 0 4px 12px rgba(79, 70, 229, 0.28) !important;',
      '  opacity: 1 !important;',
      '}',
      'button[data-testid="chat-input-send"] [data-cds-part="paint"], [data-composer-card-holder] button [data-cds-part="paint"].bg-fill-brand, [data-cds-part="paint"].bg-fill-brand, [data-cds="Button"] [data-cds-part="paint"].bg-fill-primary {',
      '  background: linear-gradient(135deg, #0D9488 0%, #4F46E5 100%) !important;',
      '  border-radius: 10px !important;',
      '  box-shadow: 0 4px 14px rgba(20, 184, 166, 0.28) !important;',
      '}',
      '[data-cds="Button"]:has([data-cds-part="paint"].bg-fill-primary) {',
      '  color: #FFFFFF !important;',
      '}',
      '[data-cds="Button"] [data-cds-part="paint"].bg-fill-secondary {',
      '  background: #FFFFFF !important;',
      '  border: 1px solid #CBD5E1 !important;',
      '  border-radius: 10px !important;',
      '}',
      '/* 4. 2-Column Grid of Elevated Cards for Active Tasks on Home & Styled DataTables on /chats */',
      'div[class*="[&>*:hover+hr]:opacity-0"] {',
      '  display: grid !important;',
      '  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)) !important;',
      '  gap: 12px !important;',
      '  margin-top: 6px !important;',
      '}',
      'div[class*="[&>*:hover+hr]:opacity-0"] > hr {',
      '  display: none !important;',
      '}',
      'div[class*="[&>*:hover+hr]:opacity-0"] > * {',
      '  background: #FFFFFF !important;',
      '  border: 1px solid #CBD5E1 !important;',
      '  border-left: 3.5px solid #14B8A6 !important;',
      '  border-radius: 12px !important;',
      '  padding: 12px 14px !important;',
      '  box-shadow: 0 3px 10px rgba(15, 23, 42, 0.04) !important;',
      '}',
      '[data-cds="DataTable"] > div {',
      '  background: #FFFFFF !important;',
      '  border: 1px solid #CBD5E1 !important;',
      '  border-radius: 14px !important;',
      '  padding: 6px 10px !important;',
      '  box-shadow: 0 6px 20px rgba(15, 23, 42, 0.05) !important;',
      '}',
      '[data-cds="Table"] tbody tr td {',
      '  padding-top: 12px !important;',
      '  padding-bottom: 12px !important;',
      '}',
      '/* 5. Card-Framed Assistant Messages & Teal-Indigo User Bubbles */',
      '[data-cds="AssistantMessage"], .font-claude-response {',
      '  background: #FFFFFF !important;',
      '  border: 1px solid #E2E8F0 !important;',
      '  border-left: 4px solid #14B8A6 !important;',
      '  border-radius: 14px !important;',
      '  padding: 16px 20px !important;',
      '  box-shadow: 0 4px 16px rgba(15, 23, 42, 0.04) !important;',
      '}',
      '[class*="bg-[var(--cds-bg-user-message)]"] {',
      '  background: linear-gradient(135deg, rgba(13, 148, 136, 0.14) 0%, rgba(79, 70, 229, 0.14) 100%) !important;',
      '  border: 1px solid rgba(20, 184, 166, 0.35) !important;',
      '  border-radius: 14px 14px 4px 14px !important;',
      '  box-shadow: 0 3px 12px rgba(15, 23, 42, 0.05) !important;',
      '}',
      '/* 6. Dark Mode ([data-mode="dark"]) Cyber-Slate Surface Adaptation */',
      '[data-mode="dark"], [data-theme="claude"][data-mode="dark"] {',
      '  --bg-000: 219 41% 14% !important;',
      '  --bg-100: 223 45% 9.2% !important;',
      '  --bg-200: 224 46% 8% !important;',
      '  --bg-300: 220 41% 13% !important;',
      '  --bg-400: 217 33% 17% !important;',
      '  --text-000: 210 40% 98% !important;',
      '  --text-100: 210 40% 96% !important;',
      '  --text-200: 213 27% 84% !important;',
      '  --text-300: 215 20% 65% !important;',
      '  --text-400: 215 16% 56% !important;',
      '  --cds-surface-page: #0D1322 !important;',
      '  --cds-surface-primary: #0D1322 !important;',
      '  --cds-surface-secondary: #131C2E !important;',
      '  --cds-surface-tertiary: #1E293B !important;',
      '  --cds-surface-popover: #151F32 !important;',
      '  --cds-text-primary: #F8FAFC !important;',
      '  --cds-text-secondary: #CBD5E1 !important;',
      '  --cds-text-tertiary: #94A3B8 !important;',
      '  --cds-text-muted: #94A3B8 !important;',
      '}',
      '[data-mode="dark"] .dframe-root {',
      '  --df-z0: 222 48% 5.3% !important;',
      '  --df-z1: 223 45% 9.2% !important;',
      '  --df-z2: 220 41% 13% !important;',
      '  --df-z3: 217 33% 17% !important;',
      '  --df-z4: 216 32% 22% !important;',
      '  --df-z5: 215 25% 27% !important;',
      '  --df-z6: 215 20% 65% !important;',
      '  --df-bg-page-hsl: 223 45% 9.2% !important;',
      '  --df-bg-page: #0D1322 !important;',
      '}',
      '[data-mode="dark"] .dframe-root .dframe-card {',
      '  --df-sidebar-bg: #131C2E !important;',
      '  background-color: #131C2E !important;',
      '  border: 1px solid #263449 !important;',
      '  border-radius: 12px !important;',
      '  box-shadow: 0 6px 20px rgba(2, 6, 23, 0.35) !important;',
      '}',
      '[data-mode="dark"] .dframe-main, [data-mode="dark"] .dframe-content {',
      '  background: radial-gradient(circle at 50% 0%, rgba(20, 184, 166, 0.10) 0%, rgba(79, 70, 229, 0.07) 38%, transparent 72%), #0D1322 !important;',
      '  color: #F8FAFC !important;',
      '}',
      '[data-mode="dark"] .dframe-header {',
      '  background: rgba(13, 19, 34, 0.86) !important;',
      '  backdrop-filter: blur(12px) !important;',
      '  border-bottom: 1px solid rgba(51, 65, 85, 0.65) !important;',
      '  color: #F8FAFC !important;',
      '}',
      '[data-mode="dark"] [data-composer-card-holder] > div {',
      '  background: #131C2E !important;',
      '  border: 1.5px solid #334155 !important;',
      '  color: #F8FAFC !important;',
      '  box-shadow: inset 0 3px 0 0 #14B8A6, 0 14px 34px -8px rgba(2, 6, 23, 0.55) !important;',
      '}',
      '[data-mode="dark"] [data-composer-card-holder] > div:focus-within {',
      '  border-color: #14B8A6 !important;',
      '  box-shadow: inset 0 3px 0 0 #4F46E5, 0 0 0 3px rgba(20, 184, 166, 0.22), 0 16px 38px -8px rgba(2, 6, 23, 0.65) !important;',
      '}',
      '[data-mode="dark"] [data-cds="Button"] [data-cds-part="paint"].bg-fill-secondary {',
      '  background: #151F32 !important;',
      '  border: 1px solid #334155 !important;',
      '  border-radius: 10px !important;',
      '}',
      '[data-mode="dark"] [data-cds="DataTable"] > div {',
      '  background: #131C2E !important;',
      '  border: 1px solid #263449 !important;',
      '  border-radius: 14px !important;',
      '  padding: 6px 10px !important;',
      '  box-shadow: 0 8px 24px rgba(2, 6, 23, 0.38) !important;',
      '}',
      '[data-mode="dark"] div[class*="[&>*:hover+hr]:opacity-0"] > * {',
      '  background: #131C2E !important;',
      '  border: 1px solid #334155 !important;',
      '  border-left: 3.5px solid #14B8A6 !important;',
      '  color: #F8FAFC !important;',
      '  box-shadow: 0 4px 14px rgba(2, 6, 23, 0.35) !important;',
      '}',
      '[data-mode="dark"] [data-cds="AssistantMessage"], [data-mode="dark"] .font-claude-response {',
      '  background: #131C2E !important;',
      '  border: 1px solid #334155 !important;',
      '  border-left: 4px solid #14B8A6 !important;',
      '  color: #F8FAFC !important;',
      '  box-shadow: 0 4px 16px rgba(2, 6, 23, 0.40) !important;',
      '}',
      '[data-mode="dark"] [class*="bg-[var(--cds-bg-user-message)]"] {',
      '  background: linear-gradient(135deg, rgba(13, 148, 136, 0.24) 0%, rgba(79, 70, 229, 0.28) 100%) !important;',
      '  border: 1px solid rgba(45, 212, 191, 0.38) !important;',
      '  border-radius: 14px 14px 4px 14px !important;',
      '  color: #F8FAFC !important;',
      '  box-shadow: 0 4px 16px rgba(2, 6, 23, 0.35) !important;',
      '}',
      '/* ========================================================================== */',
      '/* PROVIDER CONFIGURATION OVERLAY & TOP STATUS PILL                           */',
      '/* ========================================================================== */',
      '#quixi-provider-overlay {',
      '  position: fixed; inset: 0; z-index: 999990;',
      '  background: #F1F5F9; color: #0F172A;',
      '  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", Roboto, sans-serif;',
      '  display: flex; flex-direction: column; overflow: hidden;',
      '  -webkit-font-smoothing: antialiased;',
      '}',
      '#quixi-provider-overlay .qp-main, #quixi-provider-overlay .qp-card, #quixi-provider-overlay .qp-oauth-box, #quixi-provider-overlay .qp-code-pill, #quixi-provider-overlay code, #quixi-provider-overlay .qp-banner {',
      '  user-select: text; -webkit-user-select: text;',
      '}',
      '@media (prefers-color-scheme: dark) {',
      '  #quixi-provider-overlay { background: #0B1120; color: #F8FAFC; }',
      '  #quixi-provider-overlay .qp-sidebar { background: #0F172A; border-color: #1E293B; }',
      '  #quixi-provider-overlay .qp-card { background: #1E293B; border-color: #334155; }',
      '  #quixi-provider-overlay .qp-item:hover { background: #1E293B; }',
      '  #quixi-provider-overlay .qp-item.active { background: #1E293B; border-color: #14B8A6; }',
      '  #quixi-provider-overlay .qp-input, #quixi-provider-overlay .qp-select {',
      '    background: #0F172A; color: #F8FAFC; border-color: #334155;',
      '  }',
      '  #quixi-provider-overlay .qp-topbar, #quixi-provider-overlay .qp-footer {',
      '    background: #0F172A; border-color: #1E293B;',
      '  }',
      '  #quixi-provider-overlay .qp-muted { color: #94A3B8; }',
      '  #quixi-provider-overlay .qp-oauth-box { background: #0F172A; border-color: #334155; }',
      '}',
      '.qp-topbar {',
      '  height: 58px; padding: 0 24px 0 86px; border-bottom: 1px solid #CBD5E1;',
      '  background: #0F172A; color: #F8FAFC; display: flex; align-items: center; justify-content: space-between;',
      '  flex-shrink: 0;',
      '}',
      '.qp-brand { display: flex; align-items: center; gap: 12px; }',
      '.qp-logo {',
      '  width: 32px; height: 32px; border-radius: 8px;',
      '  display: flex; align-items: center; justify-content: center;',
      '  box-shadow: 0 1px 4px rgba(0,0,0,0.18); overflow: hidden; flex-shrink: 0;',
      '}',
      '.qp-title { font-size: 16px; font-weight: 650; letter-spacing: -0.01em; color: #F8FAFC; }',
      '.qp-subtitle { font-size: 12px; color: #94A3B8; }',
      '.qp-body { flex: 1; display: flex; min-height: 0; overflow: hidden; }',
      '.qp-sidebar {',
      '  width: 350px; border-right: 1px solid #CBD5E1; background: #E8EEF6;',
      '  display: flex; flex-direction: column; overflow-y: auto; padding: 14px 12px;',
      '}',
      '.qp-sec-hdr {',
      '  font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em;',
      '  color: #475569; padding: 8px 10px 6px; display: flex; align-items: center; justify-content: space-between;',
      '}',
      '.qp-item {',
      '  display: flex; align-items: center; justify-content: space-between;',
      '  padding: 10px 12px; margin-bottom: 4px; border-radius: 10px;',
      '  border: 1px solid transparent; cursor: pointer; transition: all 0.12s ease;',
      '}',
      '.qp-item:hover { background: #E2E8F0; }',
      '.qp-item.active { background: #FFFFFF; border-color: #14B8A6; box-shadow: 0 2px 6px rgba(15, 23, 42, 0.06); }',
      '.qp-item-left { display: flex; align-items: center; gap: 10px; min-width: 0; }',
      '.qp-prov-icon {',
      '  position: relative; width: 28px; height: 28px; flex-shrink: 0;',
      '  display: flex; align-items: center; justify-content: center;',
      '}',
      '.qp-rank-pip {',
      '  position: absolute; bottom: -3px; right: -4px;',
      '  min-width: 14px; height: 14px; padding: 0 3px; border-radius: 999px;',
      '  font-size: 9px; font-weight: 700; display: flex; align-items: center; justify-content: center;',
      '  background: #0F172A; color: #2DD4BF; border: 1.5px solid #FFFFFF;',
      '}',
      '.qp-item-name { font-size: 13.5px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }',
      '.qp-item-sub { font-size: 11px; color: #64748B; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 170px; }',
      '.qp-badge {',
      '  font-size: 10.5px; font-weight: 600; padding: 2px 7px; border-radius: 999px; flex-shrink: 0;',
      '  background: #E2E8F0; color: #334155;',
      '}',
      '.qp-badge.oauth { background: rgba(79, 70, 229, 0.12); color: #4F46E5; }',
      '.qp-badge.rec { background: rgba(20, 184, 166, 0.16); color: #0D9488; }',
      '.qp-badge.ok { background: rgba(16, 185, 129, 0.16); color: #059669; }',
      '.qp-main {',
      '  flex: 1; overflow-y: auto; padding: 28px 36px; display: flex; flex-direction: column; gap: 20px;',
      '  max-width: 780px;',
      '}',
      '.qp-card {',
      '  background: #FFFFFF; border: 1px solid #CBD5E1; border-radius: 14px;',
      '  padding: 22px 24px; box-shadow: 0 4px 14px rgba(15, 23, 42, 0.04);',
      '}',
      '.qp-card-hdr { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 18px; }',
      '.qp-card-title { font-size: 20px; font-weight: 650; margin: 0 0 4px 0; display: flex; align-items: center; gap: 10px; }',
      '.qp-Field { margin-bottom: 16px; }',
      '.qp-label { display: block; font-size: 12.5px; font-weight: 600; margin-bottom: 6px; }',
      '.qp-hint { font-size: 11.5px; color: #64748B; margin-top: 4px; }',
      '.qp-input, .qp-select {',
      '  width: 100%; box-sizing: border-box; padding: 9px 12px; font-size: 13.5px;',
      '  border: 1px solid #CBD5E1; border-radius: 8px; background: #F8FAFC; color: #0F172A;',
      '  outline: none; font-family: inherit;',
      '}',
      '.qp-input:focus, .qp-select:focus { border-color: #14B8A6; box-shadow: 0 0 0 2px rgba(20, 184, 166, 0.2); }',
      '.qp-tabs { display: inline-flex; background: #E2E8F0; padding: 3px; border-radius: 9px; margin-bottom: 16px; }',
      '.qp-tab {',
      '  padding: 6px 14px; font-size: 12.5px; font-weight: 600; border-radius: 7px;',
      '  cursor: pointer; border: none; background: transparent; color: #475569;',
      '}',
      '.qp-tab.active { background: #FFFFFF; color: #0F172A; box-shadow: 0 1px 3px rgba(15, 23, 42, 0.08); }',
      '.qp-oauth-box {',
      '  border: 1px solid #CBD5E1; background: #F8FAFC; border-radius: 11px; padding: 18px; margin-bottom: 16px;',
      '}',
      '.qp-btn-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-top: 10px; }',
      '.qp-btn {',
      '  padding: 9px 16px; border-radius: 8px; font-size: 13px; font-weight: 600;',
      '  cursor: pointer; border: 1px solid #CBD5E1; background: #FFFFFF; color: #0F172A;',
      '  display: inline-flex; align-items: center; gap: 7px; transition: all 0.12s ease;',
      '}',
      '.qp-btn:hover { background: #F1F5F9; }',
      '.qp-btn.primary { background: linear-gradient(135deg, #0D9488 0%, #4F46E5 100%); color: #FFFFFF; border-color: #0D9488; }',
      '.qp-btn.primary:hover { filter: brightness(1.06); }',
      '.qp-btn.oauth-btn { background: #4F46E5; color: #FFFFFF; border-color: #4338CA; }',
      '.qp-btn.oauth-btn:hover { background: #4338CA; }',
      '.qp-btn.danger { color: #B91C1C; border-color: #FCA5A5; }',
      '.qp-btn:disabled { opacity: 0.48; cursor: not-allowed; }',
      '.qp-banner {',
      '  padding: 10px 14px; border-radius: 9px; font-size: 12.5px; font-weight: 500; margin-bottom: 14px;',
      '}',
      '.qp-banner.ok { background: rgba(16, 185, 129, 0.12); color: #047857; border: 1px solid rgba(16, 185, 129, 0.3); }',
      '.qp-banner.err { background: rgba(185, 28, 28, 0.1); color: #B91C1C; border: 1px solid rgba(185, 28, 28, 0.25); }',
      '.qp-banner.info { background: rgba(79, 70, 229, 0.1); color: #4338CA; border: 1px solid rgba(79, 70, 229, 0.25); }',
      '.qp-footer {',
      '  height: 62px; padding: 0 28px; border-top: 1px solid #CBD5E1; background: #E8EEF6;',
      '  display: flex; align-items: center; justify-content: space-between; flex-shrink: 0;',
      '}',
      '.qp-code-pill {',
      '  font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 18px; font-weight: 700;',
      '  letter-spacing: 0.12em; padding: 8px 14px; border-radius: 8px; background: #FFFFFF;',
      '  border: 1px dashed #4F46E5; color: #4338CA; display: inline-block; margin: 8px 0; user-select: text;',
      '}',
      '#dframe-header-actions-slot { margin-right: 12px !important; }',
      '.animate-incognito-header-in { display: none !important; }',
      '.text-bg-000, button[aria-label="Exit incognito"] { color: #F8FAFC !important; }',
      '[data-testid="incognito-frame"] {',
      '  border: 1.5px dashed rgba(45, 212, 191, 0.55) !important;',
      '  box-shadow: 0 0 0 100px #0B1120 !important;',
      '}',
      '#quixi-provider-pill {',
      '  position: fixed; top: 7px; right: 160px; z-index: 99990;',
      '  display: inline-flex; align-items: center; gap: 7px;',
      '  padding: 4px 12px; border-radius: 8px; font-size: 11.5px; font-weight: 600;',
      '  background: #0F172A; color: #F8FAFC; border: 1px solid rgba(20, 184, 166, 0.45);',
      '  cursor: pointer; box-shadow: 0 2px 8px rgba(15, 23, 42, 0.18); backdrop-filter: blur(8px);',
      '  font-family: -apple-system, BlinkMacSystemFont, sans-serif;',
      '}',
      '#quixi-provider-pill:hover { border-color: #2DD4BF; background: #1E293B; }',
      '#quixi-browser-pill {',
      '  position: fixed; top: 7px; right: 52px; z-index: 99990;',
      '  display: inline-flex; align-items: center; gap: 6px;',
      '  padding: 4px 10px; border-radius: 8px; font-size: 11.5px; font-weight: 600;',
      '  background: #0F172A; color: #F8FAFC; border: 1px solid rgba(79, 70, 229, 0.55);',
      '  cursor: pointer; box-shadow: 0 2px 8px rgba(15, 23, 42, 0.18); backdrop-filter: blur(8px);',
      '  font-family: -apple-system, BlinkMacSystemFont, sans-serif;',
      '}',
      '#quixi-browser-pill:hover, #quixi-browser-pill.active {',
      '  border-color: #2DD4BF; background: linear-gradient(135deg, #0D9488 0%, #4F46E5 100%); color: #FFFFFF;',
      '}',
      'body.quixi-browser-open .dframe-pane-scroller, body.quixi-browser-open [data-cds="Page"], body.quixi-browser-open .dframe-main {',
      '  padding-right: 470px !important; box-sizing: border-box !important; transition: padding-right 0.14s ease;',
      '}',
      '#quixi-browser-pane {',
      '  position: fixed; top: 42px; right: 0; bottom: 0; width: 470px; z-index: 99985;',
      '  background: #0F172A; color: #F8FAFC; border-left: 1px solid #1E293B;',
      '  box-shadow: -10px 0 30px rgba(2, 6, 23, 0.28);',
      '  display: none; flex-direction: column; overflow: hidden;',
      '  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;',
      '}',
      '#quixi-browser-pane.open { display: flex; }',
      '.qb-tabbar {',
      '  display: flex; align-items: center; gap: 5px; padding: 7px 10px;',
      '  background: #0B1120; border-bottom: 1px solid #1E293B; overflow-x: auto; flex-shrink: 0;',
      '}',
      '.qb-badge {',
      '  display: inline-flex; align-items: center; gap: 5px; padding: 3px 8px; border-radius: 6px;',
      '  background: rgba(20, 184, 166, 0.16); color: #2DD4BF; font-size: 10.5px; font-weight: 700;',
      '  letter-spacing: 0.02em; flex-shrink: 0; border: 1px solid rgba(20, 184, 166, 0.32);',
      '}',
      '.qb-tab {',
      '  display: inline-flex; align-items: center; gap: 6px; padding: 4px 9px; border-radius: 7px;',
      '  font-size: 11.5px; font-weight: 500; color: #94A3B8; background: #131C2E;',
      '  border: 1px solid transparent; cursor: pointer; max-width: 150px; flex-shrink: 0;',
      '}',
      '.qb-tab.active {',
      '  color: #F8FAFC; background: #1E293B; border-color: #14B8A6;',
      '}',
      '.qb-tab-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }',
      '.qb-tab-close {',
      '  font-size: 12px; line-height: 1; color: #94A3B8; padding: 0 2px; border-radius: 4px;',
      '}',
      '.qb-tab-close:hover { color: #F8FAFC; background: rgba(255,255,255,0.12); }',
      '.qb-icon-btn {',
      '  background: #131C2E; color: #CBD5E1; border: 1px solid #334155; border-radius: 7px;',
      '  padding: 4px 8px; font-size: 12px; font-weight: 600; cursor: pointer; flex-shrink: 0;',
      '}',
      '.qb-icon-btn:hover { background: #1E293B; color: #FFFFFF; border-color: #14B8A6; }',
      '.qb-navbar {',
      '  display: flex; align-items: center; gap: 6px; padding: 7px 10px;',
      '  background: #0F172A; border-bottom: 1px solid #1E293B; flex-shrink: 0;',
      '}',
      '.qb-url-input {',
      '  flex: 1; min-width: 0; background: #0B1120; color: #F8FAFC; border: 1px solid #334155;',
      '  border-radius: 7px; padding: 5px 10px; font-size: 12px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace;',
      '  outline: none;',
      '}',
      '.qb-url-input:focus { border-color: #14B8A6; box-shadow: 0 0 0 2px rgba(20, 184, 166, 0.2); }',
      '.qb-go-btn {',
      '  background: linear-gradient(135deg, #0D9488 0%, #4F46E5 100%); color: #FFFFFF;',
      '  border: none; border-radius: 7px; padding: 5px 11px; font-size: 11.5px; font-weight: 600; cursor: pointer;',
      '}',
      '.qb-viewport {',
      '  flex: 1; min-height: 0; background: #FFFFFF; position: relative; display: flex; flex-direction: column;',
      '}',
      '#qb-iframe { flex: 1; width: 100%; height: 100%; border: none; background: #FFFFFF; }',
      '.qb-inspector {',
      '  background: #0B1120; border-top: 1px solid #1E293B; padding: 7px 10px;',
      '  max-height: 115px; overflow-y: auto; flex-shrink: 0; font-size: 11px;',
      '}',
      '.qb-inspector-hdr {',
      '  display: flex; align-items: center; justify-content: space-between; color: #94A3B8;',
      '  font-weight: 600; margin-bottom: 5px; font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.04em;',
      '}',
      '.qb-ref-list { display: flex; flex-wrap: wrap; gap: 4px; }',
      '.qb-ref-pill {',
      '  display: inline-flex; align-items: center; gap: 4px; padding: 2px 7px; border-radius: 5px;',
      '  background: #131C2E; border: 1px solid #334155; color: #E2E8F0; font-size: 10.5px; cursor: pointer;',
      '  max-width: 210px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;',
      '}',
      '.qb-ref-pill:hover { border-color: #14B8A6; background: #1E293B; }',
      '.qb-ref-tag { color: #2DD4BF; font-family: ui-monospace, monospace; font-weight: 700; }',
      '.qp-dot { width: 7px; height: 7px; border-radius: 50%; background: #2DD4BF; box-shadow: 0 0 6px #14B8A6; }',
      '.qp-dot.warn { background: #F59E0B; box-shadow: none; }',
      '[data-starting-style], [role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"], [data-cds="Dialog"], [data-cds-overlay] {',
      '  opacity: 1 !important; transform: none !important; scale: 1 !important;',
      '  transition-duration: 0s !important; transition: none !important; animation-duration: 0s !important;',
      '}',
      'button[aria-hidden="true"][class*="w-[80px]"][class*="h-[80px]"] {',
      '  display: none !important;',
      '}'
    ].join('\n');
    document.head.appendChild(style);
  }

  function getSelectedProvider() {
    if (!state || !state.providers) return null;
    for (var i = 0; i < state.providers.length; i++) {
      if (state.providers[i].id === selectedProviderId) return state.providers[i];
    }
    return state.providers[0] || null;
  }

  function dockHeaderPills() {
    if (isOverlayOpen) return;
    var pPill = document.getElementById('quixi-provider-pill');
    var bPill = document.getElementById('quixi-browser-pill');
    if ((!pPill || !bPill) && state) {
      renderPill();
      pPill = document.getElementById('quixi-provider-pill');
      bPill = document.getElementById('quixi-browser-pill');
    }
    if (!pPill || !bPill) return;
    if (pPill.parentElement !== document.body) {
      document.body.appendChild(pPill);
    }
    if (bPill.parentElement !== document.body) {
      document.body.appendChild(bPill);
    }

    var isOpen = !!(browserState && browserState.paneOpen);
    var winW = window.innerWidth || 1440;
    var hdr = document.querySelector('.dframe-header');
    var hdrRect = hdr ? hdr.getBoundingClientRect() : { left: 240, right: winW, width: winW - 240 };
    var minRightControlLeft = hdrRect.right > 200 ? hdrRect.right - 16 : winW - 54;
    var exitIncogBtn = document.querySelector('button[aria-label="Exit incognito"]');
    if (exitIncogBtn) {
      var eir = exitIncogBtn.getBoundingClientRect();
      if (eir.width > 0 && eir.left > winW - 140 && eir.left < minRightControlLeft) {
        minRightControlLeft = eir.left;
      }
    }
    var titleRight = hdrRect.left + 20;
    var hasNativeBrowserBtn = false;

    if (hdr) {
      var slot = document.getElementById('dframe-header-actions-slot');
      if (slot) {
        var sr = slot.getBoundingClientRect();
        if (sr.left > hdrRect.right - 150 && sr.left < minRightControlLeft) {
          minRightControlLeft = sr.left;
        }
      }
      var hdrBtns = hdr.querySelectorAll('button');
      for (var bi = 0; bi < hdrBtns.length; bi++) {
        var hb = hdrBtns[bi];
        if (hb.id === 'quixi-provider-pill' || hb.id === 'quixi-browser-pill') continue;
        var br = hb.getBoundingClientRect();
        if (br.width <= 0) continue;
        var ariaLbl = (hb.getAttribute('aria-label') || '');
        if (ariaLbl === 'Browser' || ariaLbl === 'Open in browser') {
          hasNativeBrowserBtn = true;
          if (!hb.__quixi_browser_hooked__) {
            hb.__quixi_browser_hooked__ = true;
            hb.addEventListener('click', function(ev) {
              ev.preventDefault();
              ev.stopPropagation();
              var currentlyOpen = !!(browserState && browserState.paneOpen);
              browserAction({ action: currentlyOpen ? 'close' : 'open' });
            }, true);
          }
        }
        if (br.left > hdrRect.right - 140) {
          if (br.left < minRightControlLeft) {
            minRightControlLeft = br.left;
          }
        } else {
          if (br.right > titleRight) {
            titleRight = br.right;
          }
        }
      }
    }

    var baseRight = isOpen ? Math.max(484, Math.round(winW - minRightControlLeft + 8)) : Math.max(16, Math.round(winW - minRightControlLeft + 8));
    var availWidth = minRightControlLeft - titleRight - 14;

    var hasCfg = state && state.hasConfiguredProvider;
    var activeProv = state ? state.activeProvider : 'openrouter';
    var activeMod = state ? state.activeModel : '';
    var shortMod = activeMod ? activeMod.split('/').pop() : activeProv;
    var provName = activeProv;
    if (state && state.providers) {
      for (var i = 0; i < state.providers.length; i++) {
        if (state.providers[i].id === activeProv) {
          provName = state.providers[i].name;
          break;
        }
      }
    }

    var desiredMode = 'full';
    if (availWidth < 135) {
      desiredMode = 'icon';
    } else if (availWidth < 310) {
      desiredMode = 'compact';
    }

    if (pPill.getAttribute('data-pill-mode') !== desiredMode) {
      pPill.setAttribute('data-pill-mode', desiredMode);
      if (desiredMode === 'icon') {
        pPill.innerHTML =
          QUIXI_MARK_SVG +
          '<span class="qp-dot ' + (hasCfg ? '' : 'warn') + '"></span>';
      } else if (desiredMode === 'compact') {
        pPill.innerHTML =
          QUIXI_MARK_SVG +
          '<span class="qp-dot ' + (hasCfg ? '' : 'warn') + '"></span>' +
          '<span>' + escapeHtml(shortMod) + '</span>';
      } else {
        pPill.innerHTML =
          QUIXI_MARK_SVG +
          '<span class="qp-dot ' + (hasCfg ? '' : 'warn') + '"></span>' +
          '<span>Providers: ' + escapeHtml(provName) + (shortMod ? ' · ' + escapeHtml(shortMod) : '') + '</span>';
      }
    }

    if (hasNativeBrowserBtn) {
      bPill.style.display = 'none';
      pPill.style.position = 'fixed';
      pPill.style.top = '7px';
      pPill.style.marginRight = '0';
      pPill.style.right = baseRight + 'px';
    } else {
      bPill.style.display = 'inline-flex';
      var bDesiredMode = (availWidth < 310) ? 'compact' : 'full';
      if (bPill.getAttribute('data-pill-mode') !== bDesiredMode) {
        bPill.setAttribute('data-pill-mode', bDesiredMode);
        if (bDesiredMode === 'compact') {
          bPill.innerHTML = '<span>🌐</span>';
        } else {
          bPill.innerHTML =
            '<span class="qp-dot"></span>' +
            '<span>🌐 Browser</span>';
        }
      }
      bPill.style.position = 'fixed';
      bPill.style.top = '7px';
      bPill.style.marginRight = '0';
      bPill.style.right = baseRight + 'px';
      var bWidth = bPill.offsetWidth || (bDesiredMode === 'compact' ? 36 : 100);
      pPill.style.position = 'fixed';
      pPill.style.top = '7px';
      pPill.style.marginRight = '0';
      pPill.style.right = (baseRight + bWidth + 6) + 'px';
    }
  }

  function renderPill() {
    injectStyles();
    var existing = document.getElementById('quixi-provider-pill');
    if (isOverlayOpen) {
      if (existing) existing.style.display = 'none';
      return;
    }
    if (!existing) {
      existing = document.createElement('button');
      existing.id = 'quixi-provider-pill';
      existing.className = 'draggable-none';
      existing.title = 'Configure LLM Providers (OpenRouter, Custom, OpenAI OAuth, Grok OAuth, Anthropic, Local)';
      existing.addEventListener('click', function(e) {
        e.preventDefault();
        e.stopPropagation();
        openOverlay(false);
      });
      document.body.appendChild(existing);
    }
    existing.style.display = 'inline-flex';
    var hasCfg = state && state.hasConfiguredProvider;
    var activeProv = state ? state.activeProvider : 'openrouter';
    var activeMod = state ? state.activeModel : '';
    var provName = activeProv;
    if (state && state.providers) {
      for (var i = 0; i < state.providers.length; i++) {
        if (state.providers[i].id === activeProv) {
          provName = state.providers[i].name;
          break;
        }
      }
    }
    existing.removeAttribute('data-pill-mode');
    existing.innerHTML =
      QUIXI_MARK_SVG +
      '<span class="qp-dot ' + (hasCfg ? '' : 'warn') + '"></span>' +
      '<span>Providers: ' + escapeHtml(provName) + (activeMod ? ' · ' + escapeHtml(activeMod.split('/').pop()) : '') + '</span>';
    renderBrowserPill();
  }

  var browserState = null;
  var lastRenderedBrowserRev = -1;
  var lastRenderedTabId = -1;

  function renderBrowserPill() {
    var btn = document.getElementById('quixi-browser-pill');
    if (isOverlayOpen) {
      if (btn) btn.style.display = 'none';
      return;
    }
    if (!btn) {
      btn = document.createElement('button');
      btn.id = 'quixi-browser-pill';
      btn.className = 'draggable-none';
      btn.title = 'Toggle In-App Browser (Claude Browser Use)';
      btn.addEventListener('click', function(e) {
        e.preventDefault();
        e.stopPropagation();
        var isOpen = browserState && browserState.paneOpen;
        browserAction({ action: isOpen ? 'close' : 'open' });
      });
      document.body.appendChild(btn);
    }
    btn.style.display = 'inline-flex';
    var isOpen = !!(browserState && browserState.paneOpen);
    btn.className = 'draggable-none' + (isOpen ? ' active' : '');
    btn.removeAttribute('data-pill-mode');
    btn.innerHTML =
      '<span class="qp-dot"></span>' +
      '<span>🌐 Browser</span>';
    dockHeaderPills();
  }

  function browserAction(payload) {
    return api('/api/quixi/browser', 'POST', payload).then(function(st) {
      if (st) {
        browserState = st;
        renderBrowserPane();
        renderBrowserPill();
      }
      return st;
    }).catch(function() {});
  }

  function renderBrowserPane() {
    injectStyles();
    var pane = document.getElementById('quixi-browser-pane');
    if (!pane) {
      pane = document.createElement('div');
      pane.id = 'quixi-browser-pane';
      pane.className = 'draggable-none';
      document.body.appendChild(pane);
    }
    var isOpen = !!(browserState && browserState.paneOpen && !isOverlayOpen);
    if (!isOpen) {
      pane.classList.remove('open');
      document.body.classList.remove('quixi-browser-open');
      return;
    }
    pane.classList.add('open');
    document.body.classList.add('quixi-browser-open');

    var tabs = (browserState && browserState.tabs) || [];
    var activeTabId = (browserState && (browserState.selectedTabId || browserState.activeTabId)) || (tabs[0] && tabs[0].tabId) || 1;
    var activeTab = (browserState && browserState.activeTab) || tabs[0] || { tabId: 1, url: 'https://example.com', title: 'Example Domain', elements: [] };

    var rev = (browserState && browserState.revision) || 0;
    if (!pane.querySelector('#qb-iframe')) {
      pane.innerHTML =
        '<div class="qb-tabbar" id="qb-tabbar"></div>' +
        '<div class="qb-navbar">' +
          '<button class="qb-icon-btn" id="qb-back-btn" title="Back">←</button>' +
          '<button class="qb-icon-btn" id="qb-fwd-btn" title="Forward">→</button>' +
          '<button class="qb-icon-btn" id="qb-reload-btn" title="Reload">↻</button>' +
          '<input type="text" class="qb-url-input" id="qb-url-input" placeholder="https://..." />' +
          '<button class="qb-go-btn" id="qb-go-btn">Go</button>' +
        '</div>' +
        '<div class="qb-viewport">' +
          '<iframe id="qb-iframe" sandbox="allow-scripts allow-same-origin allow-forms"></iframe>' +
        '</div>' +
        '<div class="qb-inspector">' +
          '<div class="qb-inspector-hdr">' +
            '<span>Accessibility DOM Refs (<span id="qb-ref-count">0</span> interactive)</span>' +
            '<span>Quixi Browser MCP Active</span>' +
          '</div>' +
          '<div class="qb-ref-list" id="qb-ref-list"></div>' +
        '</div>';

      pane.querySelector('#qb-back-btn').addEventListener('click', function() {
        browserAction({ action: 'back', tabId: activeTabId });
      });
      pane.querySelector('#qb-fwd-btn').addEventListener('click', function() {
        browserAction({ action: 'forward', tabId: activeTabId });
      });
      pane.querySelector('#qb-reload-btn').addEventListener('click', function() {
        browserAction({ action: 'reload', tabId: activeTabId });
      });
      var urlInput = pane.querySelector('#qb-url-input');
      var goBtn = pane.querySelector('#qb-go-btn');
      var triggerNav = function() {
        var u = (urlInput.value || '').trim();
        if (!u) return;
        browserAction({ action: 'navigate', url: u, tabId: activeTabId });
      };
      goBtn.addEventListener('click', triggerNav);
      urlInput.addEventListener('keydown', function(ev) {
        if (ev.key === 'Enter') {
          ev.preventDefault();
          triggerNav();
        }
      });
    }

    var tabbar = pane.querySelector('#qb-tabbar');
    if (tabbar) {
      var tabsHtml = '<span class="qb-badge"><span class="qp-dot"></span>Browser</span>';
      for (var t = 0; t < tabs.length; t++) {
        var tb = tabs[t];
        var isAct = tb.tabId === activeTabId;
        tabsHtml +=
          '<div class="qb-tab ' + (isAct ? 'active' : '') + '" data-tabid="' + tb.tabId + '">' +
            '<span class="qb-tab-title">' + escapeHtml(tb.title || tb.url || 'Tab ' + tb.tabId) + '</span>' +
            (tabs.length > 1
              ? '<span class="qb-tab-close" data-closetab="' + tb.tabId + '">×</span>'
              : '') +
          '</div>';
      }
      tabsHtml +=
        '<button class="qb-icon-btn" id="qb-newtab-btn" title="New Tab">+</button>' +
        '<div style="flex:1"></div>' +
        '<button class="qb-icon-btn" id="qb-closepane-btn" title="Close Browser Pane">✕</button>';
      tabbar.innerHTML = tabsHtml;

      var tabEls = tabbar.querySelectorAll('.qb-tab');
      for (var k = 0; k < tabEls.length; k++) {
        tabEls[k].addEventListener('click', function(ev) {
          var closeId = ev.target && ev.target.getAttribute && ev.target.getAttribute('data-closetab');
          if (closeId) {
            ev.stopPropagation();
            browserAction({ action: 'close_tab', tabId: Number(closeId) });
            return;
          }
          var tid = Number(this.getAttribute('data-tabid'));
          if (tid) browserAction({ action: 'select_tab', tabId: tid });
        });
      }
      var newTabBtn = tabbar.querySelector('#qb-newtab-btn');
      if (newTabBtn) {
        newTabBtn.addEventListener('click', function() {
          browserAction({ action: 'new_tab', url: 'https://example.com' });
        });
      }
      var closePaneBtn = tabbar.querySelector('#qb-closepane-btn');
      if (closePaneBtn) {
        closePaneBtn.addEventListener('click', function() {
          browserAction({ action: 'close' });
        });
      }
    }

    var uInput = pane.querySelector('#qb-url-input');
    if (uInput && document.activeElement !== uInput) {
      uInput.value = activeTab.url || '';
    }

    var iframe = pane.querySelector('#qb-iframe');
    if (iframe && (lastRenderedBrowserRev !== rev || lastRenderedTabId !== activeTabId)) {
      lastRenderedBrowserRev = rev;
      lastRenderedTabId = activeTabId;
      iframe.src = '/api/quixi/browser/render?tabId=' + encodeURIComponent(activeTabId) + '&rev=' + encodeURIComponent(rev);
    }

    var refList = pane.querySelector('#qb-ref-list');
    var refCount = pane.querySelector('#qb-ref-count');
    var elements = (activeTab && activeTab.elements) || [];
    var interactiveEls = elements.filter(function(el) {
      return !!(el && (el.ref_id || el.refId));
    });
    if (refCount) refCount.textContent = String(interactiveEls.length);
    if (refList) {
      var refHtml = '';
      for (var r = 0; r < Math.min(interactiveEls.length, 18); r++) {
        var el = interactiveEls[r];
        var rid = el.ref_id || el.refId || ('ref_' + (r + 1));
        var lbl = el.name || el.label || el.text || el.href || el.role || el.tag || rid;
        refHtml +=
          '<button class="qb-ref-pill" data-ref="' + escapeHtml(rid) + '" title="' + escapeHtml((el.role || el.tag || '') + ': ' + lbl) + '">' +
            '<span class="qb-ref-tag">[' + escapeHtml(rid) + ']</span>' +
            '<span>' + escapeHtml(lbl) + '</span>' +
          '</button>';
      }
      refList.innerHTML = refHtml;
      var pills = refList.querySelectorAll('.qb-ref-pill');
      for (var pIdx = 0; pIdx < pills.length; pIdx++) {
        pills[pIdx].addEventListener('click', function() {
          var refId = this.getAttribute('data-ref');
          if (refId) browserAction({ action: 'click_ref', tabId: activeTabId, ref: refId });
        });
      }
    }
  }

  function refreshBrowserState() {
    api('/api/quixi/browser', 'GET').then(function(st) {
      if (st) {
        browserState = st;
        renderBrowserPane();
        renderBrowserPill();
      }
    }).catch(function() {});
  }

  window.__quixi_open_browser__ = function(url) {
    if (url) {
      return browserAction({ action: 'navigate', url: url });
    }
    return browserAction({ action: 'open' });
  };

  function escapeHtml(s) {
    return String(s == null ? '' : s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function openOverlay(firstRun) {
    isFirstRunGate = !!firstRun;
    isOverlayOpen = true;
    renderOverlay();
    renderPill();
  }

  function closeOverlay() {
    if (oauthPollTimer) {
      clearInterval(oauthPollTimer);
      oauthPollTimer = null;
    }
    isOverlayOpen = false;
    var el = document.getElementById('quixi-provider-overlay');
    if (el) el.remove();
    renderPill();
    if (providersDirty) {
      providersDirty = false;
      try {
        if (window.indexedDB && typeof window.indexedDB.deleteDatabase === 'function') {
          window.indexedDB.deleteDatabase('keyval-store');
        }
      } catch (_) {}
      if (!window.__QUIXI_E2E_MODE__) {
        setTimeout(function() {
          window.location.reload();
        }, 60);
      }
    }
  }

  function renderProviderSidebarItem(item, activeId) {
    var isAct = item.id === activeId;
    var badgeCls = item.configured ? 'ok' : (item.supportsOauth ? 'oauth' : (item.id === 'openrouter' ? 'rec' : ''));
    var badgeTxt = item.configured ? 'Configured ✓' : item.badge;
    return (
      '<div class="qp-item draggable-none ' + (isAct ? 'active' : '') + '" data-pid="' + escapeHtml(item.id) + '">' +
        '<div class="qp-item-left">' +
          '<div class="qp-prov-icon">' +
            getProviderLogoSvg(item.id, 26) +
            '<span class="qp-rank-pip">' + item.priorityRank + '</span>' +
          '</div>' +
          '<div>' +
            '<div class="qp-item-name">' + escapeHtml(item.name) + '</div>' +
            '<div class="qp-item-sub">' + escapeHtml(item.subtitle) + '</div>' +
          '</div>' +
        '</div>' +
        '<span class="qp-badge ' + badgeCls + '">' + escapeHtml(badgeTxt) + '</span>' +
      '</div>'
    );
  }

  function renderOverlay() {
    injectStyles();
    if (!state || !state.providers) return;

    var root = document.getElementById('quixi-provider-overlay');
    if (!root) {
      root = document.createElement('div');
      root.id = 'quixi-provider-overlay';
      document.body.appendChild(root);
    }

    var p = getSelectedProvider();
    if (!p) return;

    var priorityProviders = state.providers.filter(function(item) { return item.priorityRank <= 5; });
    var otherProviders = state.providers.filter(function(item) { return item.priorityRank > 5; });

    var sidebarHtml = '<div class="qp-sec-hdr"><span>Primary Providers (Priority Order)</span></div>';
    priorityProviders.forEach(function(item) {
      sidebarHtml += renderProviderSidebarItem(item, p.id);
    });

    sidebarHtml +=
      '<div class="qp-sec-hdr" style="margin-top:12px;">' +
        '<span>Cloud & Local Providers (HexisPro)</span>' +
        '<button id="qp-detect-local-btn" class="qp-btn draggable-none" style="padding:2px 8px;font-size:10.5px;">Scan Local</button>' +
      '</div>';

    otherProviders.forEach(function(item) {
      sidebarHtml += renderProviderSidebarItem(item, p.id);
    });

    var bannerHtml = '';
    if (statusBanner) {
      bannerHtml = '<div class="qp-banner ' + statusBanner.kind + '">' + escapeHtml(statusBanner.text) + '</div>';
    }

    var authSectionHtml = '';
    if (p.supportsOauth) {
      var mode = p.authMode || 'oauth';
      authSectionHtml +=
        '<div class="qp-tabs draggable-none">' +
          '<button class="qp-tab ' + (mode === 'oauth' ? 'active' : '') + '" data-authmode="oauth">' +
            escapeHtml(p.oauthLabel || 'OAuth Sign-In') + ' (Recommended)' +
          '</button>' +
          '<button class="qp-tab ' + (mode !== 'oauth' ? 'active' : '') + '" data-authmode="api_key">' +
            'API Key (' + escapeHtml(p.envVar) + ')' +
          '</button>' +
        '</div>';

      if (mode === 'oauth') {
        var activeOAuth = oauthSession && oauthSession.providerId === p.id ? oauthSession : null;
        authSectionHtml +=
          '<div class="qp-oauth-box">' +
            '<div style="font-weight:600;font-size:14px;margin-bottom:6px;">' +
              (p.id === 'openai'
                ? 'ChatGPT Plus / Pro Subscription (OpenAI Codex OAuth)'
                : 'SuperGrok / X Account (xAI Device OAuth)') +
            '</div>' +
            '<div class="qp-muted" style="font-size:12.5px;margin-bottom:12px;">' +
              (p.id === 'openai'
                ? 'Authenticate directly with your ChatGPT account using PKCE / Device Code OAuth without needing a separate platform API key.'
                : 'Authenticate directly with your X / SuperGrok account via xAI RFC 8628 Device Code OAuth.') +
            '</div>' +
            (p.configured && p.oauthAccountLabel
              ? '<div class="qp-banner ok">Connected via OAuth: <strong>' + escapeHtml(p.oauthAccountLabel) + '</strong></div>'
              : '') +
            (activeOAuth
              ? '<div style="margin:10px 0;padding:12px;border-radius:8px;background:rgba(37,99,235,0.06);">' +
                  '<div style="font-size:12px;font-weight:600;">1. Enter this verification code in your browser:</div>' +
                  '<div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin:6px 0;">' +
                    '<div id="qp-oauth-code-pill" class="qp-code-pill" title="Click or press ⌘C to copy">' + escapeHtml(activeOAuth.userCode) + '</div>' +
                    '<button id="qp-oauth-copy-code-btn" class="qp-btn draggable-none" style="padding:7px 12px;font-size:12px;">Copy Code (⌘C)</button>' +
                  '</div>' +
                  '<div style="font-size:12px;margin-top:6px;display:flex;align-items:center;gap:8px;flex-wrap:wrap;">' +
                    '<span>Verification URL: <code>' + escapeHtml(activeOAuth.verificationUrl) + '</code></span>' +
                    '<button id="qp-oauth-copy-url-btn" class="qp-btn draggable-none" style="padding:3px 9px;font-size:11px;">Copy URL</button>' +
                  '</div>' +
                  '<div class="qp-btn-row" style="margin-top:12px;">' +
                    '<button id="qp-oauth-complete-btn" class="qp-btn oauth-btn draggable-none">Complete OAuth Sign-In ✓</button>' +
                  '</div>' +
                '</div>'
              : '<div class="qp-btn-row">' +
                  '<button id="qp-oauth-start-btn" class="qp-btn oauth-btn draggable-none">' +
                    '↗ ' + escapeHtml(p.oauthLabel || 'Sign in with OAuth') +
                  '</button>' +
                '</div>') +
          '</div>';
      } else {
        authSectionHtml += renderApiKeyInput(p);
      }
    } else if (p.id === 'custom') {
      authSectionHtml +=
        '<div class="qp-Field">' +
          '<label class="qp-label">Endpoint Base URL</label>' +
          '<input id="qp-baseurl-input" class="qp-input draggable-none" type="text" value="' + escapeHtml(p.baseUrl) + '" placeholder="https://your-endpoint.example.com/v1" />' +
          '<div class="qp-hint">Works with LiteLLM, vLLM, OpenRouter proxies, Ollama, or any OpenAI / Anthropic compatible server.</div>' +
        '</div>' +
        '<div class="qp-Field">' +
          '<label class="qp-label">Wire API Format</label>' +
          '<select id="qp-apiformat-select" class="qp-select draggable-none">' +
            '<option value="openai" ' + (p.apiFormat === 'openai' ? 'selected' : '') + '>OpenAI Compatible (/v1/chat/completions)</option>' +
            '<option value="anthropic" ' + (p.apiFormat === 'anthropic' ? 'selected' : '') + '>Anthropic Messages (/v1/messages)</option>' +
          '</select>' +
        '</div>' +
        renderApiKeyInput(p) +
        '<div class="qp-Field">' +
          '<label class="qp-label">Custom Model ID</label>' +
          '<input id="qp-custommodel-input" class="qp-input draggable-none" type="text" value="' + escapeHtml(p.selectedModel) + '" placeholder="e.g. deepseek-r1, claude-sonnet-4-6, qwen2.5-coder" />' +
        '</div>';
    } else {
      if (p.id === 'anthropic') {
        authSectionHtml +=
          '<div class="qp-banner info" style="margin-bottom:14px;">' +
            'Anthropic does not support third-party consumer OAuth. Enter an Anthropic Console API key (<code>sk-ant-...</code>) or use <strong>OpenRouter (#1)</strong>.' +
          '</div>';
      }
      authSectionHtml += renderApiKeyInput(p);
      if (p.category === 'local') {
        authSectionHtml +=
          '<div class="qp-Field">' +
            '<label class="qp-label">Local Server Base URL</label>' +
            '<input id="qp-baseurl-input" class="qp-input draggable-none" type="text" value="' + escapeHtml(p.baseUrl) + '" />' +
          '</div>';
      }
    }

    var modelsOptionsHtml = '';
    (p.models || []).forEach(function(m) {
      if (p.id === 'openai') {
        var isOauthMode = (p.authMode || 'oauth') === 'oauth';
        if (isOauthMode && (m.label || '').indexOf('(Platform API)') !== -1) return;
        if (!isOauthMode && (m.label || '').indexOf('(Codex OAuth)') !== -1) return;
      }
      var sel = m.id === p.selectedModel ? 'selected' : '';
      modelsOptionsHtml += '<option value="' + escapeHtml(m.id) + '" ' + sel + '>' + escapeHtml(m.label) + ' (' + escapeHtml(m.id) + ')</option>';
    });

    var configuredCount = state.configuredCount || 0;

    root.innerHTML =
      '<div class="qp-topbar draggable">' +
        '<div class="qp-brand">' +
          '<div class="qp-logo" id="qp-brand-badge">' + QUIXI_APP_ICON_SVG + '</div>' +
          '<div>' +
            '<div class="qp-title">QuixiDesk — LLM Provider Configuration</div>' +
            '<div class="qp-subtitle">' +
              (isFirstRunGate
                ? 'First-Run Setup: Configure at least one LLM provider to unlock QuixiDesk'
                : 'Manage cloud, OAuth, custom endpoint, and local LLM providers') +
            '</div>' +
          '</div>' +
        '</div>' +
        '<div class="qp-btn-row draggable-none" style="margin-top:0;">' +
          '<span class="qp-badge ' + (configuredCount > 0 ? 'ok' : 'rec') + '" style="font-size:12px;padding:5px 11px;">' +
            configuredCount + ' Provider' + (configuredCount === 1 ? '' : 's') + ' Configured' +
          '</span>' +
          (!isFirstRunGate || configuredCount > 0
            ? '<button id="qp-close-top-btn" class="qp-btn">Back to Workspace ✕</button>'
            : '') +
        '</div>' +
      '</div>' +
      '<div class="qp-body">' +
        '<div class="qp-sidebar draggable-none">' + sidebarHtml + '</div>' +
        '<div class="qp-main draggable-none">' +
          bannerHtml +
          '<div class="qp-card">' +
            '<div class="qp-card-hdr">' +
              '<div>' +
                '<h2 class="qp-card-title">' +
                  getProviderLogoSvg(p.id, 30) +
                  '<span>' + escapeHtml(p.name) + '</span>' +
                  '<span class="qp-badge ' + (p.configured ? 'ok' : (p.supportsOauth ? 'oauth' : 'rec')) + '">' +
                    escapeHtml(p.configured ? 'Configured ✓' : p.badge) +
                  '</span>' +
                '</h2>' +
                '<div class="qp-muted" style="font-size:13px;">' + escapeHtml(p.subtitle) + '</div>' +
              '</div>' +
            '</div>' +
            authSectionHtml +
            '<div class="qp-Field">' +
              '<label class="qp-label">Default Model for ' + escapeHtml(p.name) + '</label>' +
              '<select id="qp-model-select" class="qp-select draggable-none">' + modelsOptionsHtml + '</select>' +
            '</div>' +
            '<div class="qp-btn-row">' +
              '<button id="qp-save-btn" class="qp-btn primary draggable-none">' +
                'Save & Set Active (' + escapeHtml(p.name) + ')' +
              '</button>' +
              '<button id="qp-test-btn" class="qp-btn draggable-none">Test Connection</button>' +
              (p.configured
                ? '<button id="qp-remove-btn" class="qp-btn danger draggable-none">Disconnect</button>'
                : '') +
            '</div>' +
          '</div>' +
        '</div>' +
      '</div>' +
      '<div class="qp-footer draggable-none">' +
        '<div class="qp-muted" style="font-size:12.5px;">' +
          (configuredCount > 0
            ? 'Active default: <strong>' + escapeHtml(state.activeProvider) + '</strong> · <code>' + escapeHtml(state.activeModel) + '</code>'
            : 'Select a provider above (starting with OpenRouter, Custom Endpoint, OpenAI OAuth, Grok OAuth, or Anthropic) to continue.') +
        '</div>' +
        '<div class="qp-btn-row" style="margin-top:0;">' +
          '<button id="qp-continue-btn" class="qp-btn primary draggable-none" style="padding:10px 22px;font-size:14px;">' +
            (configuredCount > 0 ? 'Continue to QuixiDesk →' : 'Save & Continue to QuixiDesk →') +
          '</button>' +
        '</div>' +
      '</div>';

    bindOverlayEvents(root, p);
  }

  function renderApiKeyInput(p) {
    return (
      '<div class="qp-Field">' +
        '<label class="qp-label">API Key (' + escapeHtml(p.envVar) + ')</label>' +
        '<input id="qp-apikey-input" class="qp-input draggable-none" type="password" ' +
          'value="" ' +
          'placeholder="' + escapeHtml(p.apiKeyMasked ? 'Saved: ' + p.apiKeyMasked + ' (enter new key to replace)' : p.keyPlaceholder) + '" />' +
        (p.docsUrl
          ? '<div class="qp-hint">Get a key from <span style="text-decoration:underline;cursor:pointer;" data-exturl="' + escapeHtml(p.docsUrl) + '">' + escapeHtml(p.docsUrl) + '</span></div>'
          : '') +
      '</div>'
    );
  }

  function collectFormPayload(p, completeOnboarding) {
    var keyEl = document.getElementById('qp-apikey-input');
    var urlEl = document.getElementById('qp-baseurl-input');
    var fmtEl = document.getElementById('qp-apiformat-select');
    var modEl = document.getElementById('qp-model-select');
    var customModEl = document.getElementById('qp-custommodel-input');

    return {
      providerId: p.id,
      apiKey: keyEl ? keyEl.value : '',
      baseUrl: urlEl ? urlEl.value : p.baseUrl,
      apiFormat: fmtEl ? fmtEl.value : p.apiFormat,
      authMode: p.authMode,
      selectedModel: customModEl && customModEl.value ? customModEl.value : (modEl ? modEl.value : p.selectedModel),
      customModel: customModEl ? customModEl.value : '',
      setAsActive: true,
      completeOnboarding: !!completeOnboarding
    };
  }

  function bindOverlayEvents(root, p) {
    var items = root.querySelectorAll('.qp-item[data-pid]');
    items.forEach(function(el) {
      el.addEventListener('click', function() {
        selectedProviderId = el.getAttribute('data-pid');
        statusBanner = null;
        renderOverlay();
      });
    });

    var tabs = root.querySelectorAll('.qp-tab[data-authmode]');
    tabs.forEach(function(tab) {
      tab.addEventListener('click', function() {
        p.authMode = tab.getAttribute('data-authmode');
        statusBanner = null;
        renderOverlay();
      });
    });

    var saveBtn = document.getElementById('qp-save-btn');
    if (saveBtn) {
      saveBtn.addEventListener('click', function() {
        var payload = collectFormPayload(p, false);
        api('/api/quixi/providers/save', 'POST', payload).then(function(newState) {
          state = newState;
          providersDirty = true;
          statusBanner = {
            kind: 'ok',
            text: p.name + ' saved and set as active provider (' + newState.activeModel + ').'
          };
          renderOverlay();
        });
      });
    }

    var testBtn = document.getElementById('qp-test-btn');
    if (testBtn) {
      testBtn.addEventListener('click', function() {
        statusBanner = { kind: 'info', text: 'Testing connection to ' + p.name + '…' };
        renderOverlay();
        var payload = collectFormPayload(p, false);
        api('/api/quixi/providers/test', 'POST', payload).then(function(res) {
          statusBanner = {
            kind: res.ok ? 'ok' : 'err',
            text: (res.message || 'Connection test finished') + ' (' + (res.latencyMs || 0) + ' ms)'
          };
          return api('/api/quixi/providers', 'GET');
        }).then(function(newState) {
          state = newState;
          renderOverlay();
        });
      });
    }

    var removeBtn = document.getElementById('qp-remove-btn');
    if (removeBtn) {
      removeBtn.addEventListener('click', function() {
        api('/api/quixi/providers/remove', 'POST', { providerId: p.id }).then(function(newState) {
          state = newState;
          providersDirty = true;
          statusBanner = { kind: 'info', text: 'Disconnected ' + p.name + '.' };
          renderOverlay();
        });
      });
    }

    var detectBtn = document.getElementById('qp-detect-local-btn');
    if (detectBtn) {
      detectBtn.addEventListener('click', function(e) {
        e.stopPropagation();
        statusBanner = { kind: 'info', text: 'Scanning localhost ports (:11434 Ollama, :1234 LM Studio, :8000 vLLM, :8080 llama.cpp)…' };
        renderOverlay();
        api('/api/quixi/providers/detect-local', 'POST', {}).then(function(res) {
          state = res.state;
          var found = (res.servers || []).filter(function(s) { return s.reachable; });
          if (found.length > 0) providersDirty = true;
          statusBanner = {
            kind: found.length > 0 ? 'ok' : 'info',
            text: found.length > 0
              ? 'Detected ' + found.length + ' running local server(s): ' + found.map(function(s) { return s.name; }).join(', ')
              : 'No local servers detected on ports 11434, 1234, 8000, or 8080.'
          };
          renderOverlay();
        });
      });
    }

    var oauthStartBtn = document.getElementById('qp-oauth-start-btn');
    if (oauthStartBtn) {
      oauthStartBtn.addEventListener('click', function() {
        statusBanner = { kind: 'info', text: 'Starting OAuth flow for ' + p.name + '…' };
        renderOverlay();
        api('/api/quixi/providers/oauth/start', 'POST', { providerId: p.id, openBrowser: true }).then(function(res) {
          oauthSession = res;
          if (res && res.userCode && typeof window.__quixi_copy_text__ === 'function') {
            window.__quixi_copy_text__(res.userCode);
          }
          statusBanner = {
            kind: 'info',
            text: 'Browser opened for ' + p.name + ' OAuth. Code ' + res.userCode + ' copied to clipboard — paste (⌘V) in your browser.'
          };
          renderOverlay();
          if (oauthPollTimer) clearInterval(oauthPollTimer);
          oauthPollTimer = setInterval(function() {
            if (!oauthSession) {
              clearInterval(oauthPollTimer);
              oauthPollTimer = null;
              return;
            }
            api('/api/quixi/providers/oauth/poll', 'POST', {
              providerId: oauthSession.providerId,
              completeNow: false
            }).then(function(pollRes) {
              if (pollRes && pollRes.status === 'authorized') {
                clearInterval(oauthPollTimer);
                oauthPollTimer = null;
                if (pollRes.state) state = pollRes.state;
                providersDirty = true;
                oauthSession = null;
                statusBanner = {
                  kind: 'ok',
                  text: p.name + ' connected via OAuth (' + (pollRes.accountLabel || 'Authorized') + ').'
                };
                renderOverlay();
              }
            }).catch(function() {});
          }, 3000);
        });
      });
    }

    var oauthCopyCodeBtn = document.getElementById('qp-oauth-copy-code-btn');
    var oauthCodePill = document.getElementById('qp-oauth-code-pill');
    function copyOAuthCode() {
      if (!oauthSession || !oauthSession.userCode) return;
      if (typeof window.__quixi_copy_text__ === 'function') {
        window.__quixi_copy_text__(oauthSession.userCode);
      } else if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(oauthSession.userCode);
      }
      if (oauthCopyCodeBtn) {
        oauthCopyCodeBtn.textContent = 'Copied ' + oauthSession.userCode + ' ✓';
      }
      statusBanner = {
        kind: 'ok',
        text: 'Copied verification code ' + oauthSession.userCode + ' to clipboard.'
      };
    }
    if (oauthCopyCodeBtn) {
      oauthCopyCodeBtn.addEventListener('click', copyOAuthCode);
    }
    if (oauthCodePill) {
      oauthCodePill.addEventListener('click', copyOAuthCode);
    }

    var oauthCopyUrlBtn = document.getElementById('qp-oauth-copy-url-btn');
    if (oauthCopyUrlBtn) {
      oauthCopyUrlBtn.addEventListener('click', function() {
        if (!oauthSession || !oauthSession.verificationUrl) return;
        if (typeof window.__quixi_copy_text__ === 'function') {
          window.__quixi_copy_text__(oauthSession.verificationUrl);
        } else if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(oauthSession.verificationUrl);
        }
        oauthCopyUrlBtn.textContent = 'Copied URL ✓';
      });
    }

    var oauthCompleteBtn = document.getElementById('qp-oauth-complete-btn');
    if (oauthCompleteBtn) {
      oauthCompleteBtn.addEventListener('click', function() {
        api('/api/quixi/providers/oauth/poll', 'POST', {
          providerId: p.id,
          completeNow: true
        }).then(function(res) {
          if (res && res.status === 'authorized') {
            if (oauthPollTimer) {
              clearInterval(oauthPollTimer);
              oauthPollTimer = null;
            }
            if (res.state) state = res.state;
            providersDirty = true;
            oauthSession = null;
            statusBanner = {
              kind: 'ok',
              text: p.name + ' connected via OAuth (' + (res.accountLabel || 'Authorized') + ').'
            };
          } else {
            statusBanner = {
              kind: 'info',
              text: 'Waiting for browser authorization of code ' + (oauthSession ? oauthSession.userCode : '') + '…'
            };
          }
          renderOverlay();
        });
      });
    }

    var closeTopBtn = document.getElementById('qp-close-top-btn');
    if (closeTopBtn) {
      closeTopBtn.addEventListener('click', function() {
        closeOverlay();
      });
    }

    var continueBtn = document.getElementById('qp-continue-btn');
    if (continueBtn) {
      continueBtn.addEventListener('click', function() {
        var payload = collectFormPayload(p, true);
        api('/api/quixi/providers/save', 'POST', payload).then(function(newState) {
          state = newState;
          providersDirty = true;
          return api('/api/quixi/providers/complete-onboarding', 'POST', {
            activeProvider: state.activeProvider,
            activeModel: state.activeModel
          });
        }).then(function(finalState) {
          state = finalState;
          closeOverlay();
        });
      });
    }
  }

  function refreshProviderPillState() {
    api('/api/quixi/providers', 'GET').then(function(st) {
      if (st) {
        state = st;
        renderPill();
      }
    }).catch(function() {});
  }

  function installModelSyncHook() {
    try {
      var prevFetch = window.fetch;
      if (prevFetch && !prevFetch.__quixi_model_hook__) {
        var hookedFetch = function(input, init) {
          var urlStr = typeof input === 'string' ? input : (input && input.url ? input.url : '');
          var p = prevFetch.apply(this, arguments);
          if (urlStr.indexOf('/model_selector_state/') !== -1) {
            p.then(function() { setTimeout(refreshProviderPillState, 40); }).catch(function() {});
          } else if (urlStr.indexOf('/__eipc__/invoke') !== -1 && init && typeof init.body === 'string' && init.body.indexOf('_$_setModel') !== -1) {
            p.then(function() { setTimeout(refreshProviderPillState, 40); }).catch(function() {});
          } else if (urlStr.indexOf('/api/account_profile') !== -1 && init && (init.method === 'PUT' || init.method === 'PATCH' || init.method === 'POST')) {
            try {
              var bodyObj = typeof init.body === 'string' ? JSON.parse(init.body) : null;
              if (bodyObj && bodyObj.locale) {
                if (window['claude.hybrid'] && window['claude.hybrid'].DesktopIntl) {
                  window['claude.hybrid'].DesktopIntl.requestLocaleChange(bodyObj.locale);
                }
                p.then(function() {
                  if (!window.__QUIXI_E2E_MODE__) {
                    setTimeout(function() { window.location.reload(); }, 120);
                  }
                }).catch(function() {});
              }
            } catch (_) {}
          }
          return p;
        };
        hookedFetch.__quixi_model_hook__ = true;
        window.fetch = hookedFetch;
      }
    } catch (_) {}
  }

  function hydrateLocalConnectorsOnBoot() {
    try {
      import('/assets/v1/shared-156-BLTosN8U.js').then(function(mod) {
        if (!mod || typeof mod.yp !== 'function') return;
        var eipc = window['claude.web'];
        if (!eipc || !eipc.LocalAgentModeSessions || typeof eipc.LocalAgentModeSessions.getLocalMcpServers !== 'function') return;
        eipc.LocalAgentModeSessions.getLocalMcpServers().then(function(servers) {
          if (!Array.isArray(servers)) return;
          for (var i = 0; i < servers.length; i++) {
            var srv = servers[i];
            if (!srv || !srv.name) continue;
            mod.yp(srv.name, {}, srv.uuid || srv.name, !!srv.isInternal);
          }
        }).catch(function() {});
      }).catch(function() {});
    } catch (_) {}
  }

  function installBrowserListeners() {
    try {
      if (window.__QUIXI_LISTENERS__) {
        if (!window.__QUIXI_LISTENERS__.has('quixi:browser-updated')) {
          window.__QUIXI_LISTENERS__.set('quixi:browser-updated', new Set());
        }
        window.__QUIXI_LISTENERS__.get('quixi:browser-updated').add(function(_ev, st) {
          if (st) {
            browserState = st;
            renderBrowserPane();
            renderBrowserPill();
          } else {
            refreshBrowserState();
          }
        });
      }
      window.addEventListener('message', function(ev) {
        var d = ev && ev.data;
        if (!d || typeof d !== 'object') return;
        if (d.type === 'quixi-browser-navigate' && d.url) {
          browserAction({ action: 'navigate', url: d.url });
        } else if (d.type === 'quixi-browser-click-ref' && d.ref) {
          browserAction({ action: 'click_ref', ref: d.ref });
        }
      });
    } catch (_) {}
  }

  function bootProviderCheck() {
    startLogoGuard();
    installModelSyncHook();
    installBrowserListeners();
    refreshBrowserState();
    setTimeout(hydrateLocalConnectorsOnBoot, 600);
    api('/api/quixi/providers', 'GET').then(function(st) {
      state = st;
      if (st && st.activeProvider) {
        selectedProviderId = st.activeProvider;
      }
      if (!st.onboardingCompleted || !st.hasConfiguredProvider) {
        openOverlay(true);
      } else {
        renderPill();
      }
    }).catch(function() {});
  }

  window.openQuixiProviderSettings = function() {
    openOverlay(false);
  };

  window.addEventListener('keydown', function(e) {
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && (e.key === 'P' || e.key === 'p')) {
      e.preventDefault();
      if (isOverlayOpen) closeOverlay();
      else openOverlay(false);
    }
  });

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', bootProviderCheck);
  } else {
    setTimeout(bootProviderCheck, 50);
  }
})();
