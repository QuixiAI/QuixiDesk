// The SDK's CAPTURE_WORKLET_SOURCE, word for word: session.test.ts fails once the two differ.
class VoiceSdkCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.port.onmessage = (event) => this.port.postMessage(event.data);
  }
  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (channel && channel.length > 0) {
      const block = channel.slice();
      this.port.postMessage(block, [block.buffer]);
    }
    return true;
  }
}
registerProcessor("voice-sdk-capture-2", VoiceSdkCapture);
