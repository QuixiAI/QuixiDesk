/* Attachment state belongs to a draft, never to the currently visible conversation. */
(() => {
  "use strict";
  const limits = Object.freeze({files: 8, imageBytes: 5 * 1024 * 1024, textBytes: 1024 * 1024, totalBytes: 20 * 1024 * 1024});
  const accept = "image/png,image/jpeg,image/webp,image/gif,.txt,.md,.csv,.tsv,.json,.yaml,.yml,.xml,.html,.css,.js,.mjs,.cjs,.ts,.tsx,.jsx,.rs,.py,.go,.java,.c,.h,.cpp,.sh,.toml,.ini,.log,.sql,.svg";
  function detectedImage(bytes) {
    const matches = (offset, values) => values.every((value, i) => bytes[offset + i] === value);
    if (matches(0, [137,80,78,71,13,10,26,10])) return "image/png";
    if (matches(0, [255,216,255])) return "image/jpeg";
    if (matches(0, [71,73,70,56]) && [55,57].includes(bytes[4]) && bytes[5] === 97) return "image/gif";
    if (matches(0, [82,73,70,70]) && matches(8, [87,69,66,80])) return "image/webp";
    return null;
  }
  function encode(bytes) {
    let binary = "";
    for (let i = 0; i < bytes.length; i += 16384) binary += String.fromCharCode(...bytes.subarray(i, i + 16384));
    return btoa(binary);
  }
  async function load(item, onChange) {
    const token = {};
    item.token = token; item.status = "loading"; item.error = null;
    onChange?.();
    try {
      if (item.file.size > limits.imageBytes) throw new Error("File exceeds the 5 MB image / 1 MB text limit.");
      const bytes = new Uint8Array(await item.file.arrayBuffer());
      if (item.token !== token) return;
      const image = detectedImage(bytes);
      if (image) {
        item.mimeType = image; item.data = encode(bytes); item.size = bytes.length;
        item.preview = `data:${image};base64,${item.data}`;
      } else {
        if (item.file.type.startsWith("image/") && item.file.type !== "image/svg+xml") throw new Error("Use a PNG, JPEG, GIF, or WebP image.");
        if (bytes.length > limits.textBytes) throw new Error("Text files must be 1 MB or smaller.");
        if (bytes.includes(0)) throw new Error("This file is not supported. Attach an image or UTF-8 text file.");
        // PDFs, office archives, and other binaries must never be silently sent as gibberish.
        const prefix = new TextDecoder().decode(bytes.subarray(0,8));
        if (prefix.startsWith("%PDF-") || prefix.startsWith("PK") || (item.file.type && !item.file.type.startsWith("text/") && !["application/json","application/xml","application/javascript","application/x-yaml","image/svg+xml"].includes(item.file.type))) {
          throw new Error("Unsupported file format. Attach PNG, JPEG, GIF, WebP, or UTF-8 text.");
        }
        try { item.text = new TextDecoder("utf-8", {fatal: true}).decode(bytes); }
        catch { throw new Error("This file is not valid UTF-8 text."); }
        item.mimeType = "text/plain"; item.size = bytes.length;
      }
      item.status = "ready";
    } catch (error) {
      if (item.token !== token) return;
      item.status = "error"; item.error = error.message || "Could not read this file.";
    }
    onChange?.();
  }
  function clipboardImages(clipboard) {
    if (!clipboard) return [];
    const items = Array.from(clipboard.items || []).filter(item => item.kind === "file" && item.type.startsWith("image/"));
    const files = items.map(item => item.getAsFile()).filter(Boolean);
    // Some WebKit clipboard implementations expose only files, not items.
    return files.length ? files : Array.from(clipboard.files || []).filter(file => file.type.startsWith("image/"));
  }
  function queueFiles(files, onChange) {
    const items = Array.from(files).map(file => ({id: crypto.randomUUID(), name: file.name || "clipboard-image", file, size: file.size, status: "loading"}));
    // Start after the caller has attached these objects to the originating draft.
    queueMicrotask(() => items.forEach(item => { if (item.status !== "cancelled") void load(item, onChange); }));
    return items;
  }
  function cancelAttachment(item) { item.token = null; item.status = "cancelled"; delete item.data; delete item.preview; }
  function attachmentsForSend(items) {
    if (items.length > limits.files) throw new Error("Attach up to 8 files per message.");
    if (items.some(item => item.status === "loading")) throw new Error("Wait for attachments to finish loading.");
    const failed = items.find(item => item.status && item.status !== "ready");
    if (failed) throw new Error(`${failed.name}: ${failed.error || "Reattach or remove this file before sending."}`);
    if (items.reduce((sum, item) => sum + (item.size ?? new TextEncoder().encode(item.text || "").length), 0) > limits.totalBytes) throw new Error("Attachments exceed the 20 MB total limit.");
    return items.map(item => ({name: item.name, mimeType: item.mimeType || "text/plain", ...(item.data ? {data: item.data} : {text: item.text ?? ""})}));
  }
  function editableText(message) {
    return (message?.message?.content || []).filter(block => block.type === "text" && !block.attachment).map(block => block.text || "").join("\n");
  }
  function transcriptAttachments(message) {
    return (message?.message?.content || []).filter(block => block.attachment).map(block => ({...block.attachment, preview: block.type === "image_url" ? block.image_url?.url : undefined}));
  }
  window.QuixiChat = Object.freeze({limits, accept, clipboardImages, queueFiles, cancelAttachment, retryAttachment: load, attachmentsForSend, editableText, transcriptAttachments});
})();
