import{n as e}from"./preload-helper-D0dM6GpB.js";import{Ba as t,Co as n,Ha as r,Ia as i,Ua as a,Va as o,ho as s,so as c,wo as l}from"./vendor-frame-CnSY1sxv.js";import{$f as J2,Al as u,Dl as d,Eu as f,Iu as p,Jf as I3,Kf as I1,Kl as m,Qf as J1,Xf as I5,Zf as J0,ap as K0,cp as K3,dp as Z1,ep as J3,ip as J9,lp as K4,np as J5,od as h,op as K1,pp as Z3,qf as I2,rp as J8,sp as K2,tp as J4,up as K5}from"./shared-frame-6jtSkIit.js";import{cp as g,lp as _,sp as v}from"./shared-25-DOWZRSlq.js";import{Id as y,gm as b,tn as x}from"./shared-0-DDOOdL6P.js";import{En as S}from"./shared-6-sd9jbJqs.js";import{i as ee,m as te,r as C}from"./vendor-motion-DlXQpU4V.js";import{n as w,t as T}from"./c0ce8f953-DPnHAvZz.js";import{t as ne}from"./ca74e9c4f-Bd6thou8.js";import{l as re,p as ie}from"./vendor-intl-DYfox5Co.js";import{Ci as ae,Gi as E,Hi as oe,Oi as se,Si as ce,Ti as le,Ui as D,xi as ue}from"./shared-182-BI1KJccu.js";import{n as de}from"./c618e268c-ByOzEgJd.js";import{t as fe}from"./c3a5d48a4-DYZwAywx.js";var O="__cowork_html_review_ready__",pe="__cwhr_draft",me=1e3,k=`url("data:image/svg+xml,%3Csvg%20xmlns%3D'http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg'%20width%3D'24'%20height%3D'24'%20viewBox%3D'0%200%2024%2024'%3E%3Cpath%20d%3D'M12%20.5h0A11.5%2011.5%200%200%201%2023.5%2012v0A11.5%2011.5%200%200%201%2012%2023.5H2.5a2%202%200%200%201-2-2V12A11.5%2011.5%200%200%201%2012%20.5Z'%20fill%3D'%23fff'%20stroke%3D'rgba(0%2C0%2C0%2C0.22)'%20stroke-width%3D'1'%2F%3E%3C%2Fsvg%3E") 1 23, url("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACjklEQVR4AbRWS2gaURR9TGpiFTUEWpKQkA+EruqizWcRSBRT/G8KKpXiyo0ra1fVNiDFLmypFUo3iht/iO4Uay1FGwtFECl1p6KWLly4CGqrSMTY+wRFQ7WhGYd7Zu697845b+7M8B6BJh+UxcXFG3a7XeTxeHQAC+C12+1+ZLPZhHgMbp8FjLWxAkaj8U48Hg8lEomfIpEoLBaLLTKZTAd4LJFI3kD8HsZ+xGKxoMFgYI9T+JvAtUAg8FylUn3hcDj8ra0t6vLyMmKxWIhCofSAfZyDMRqXy+Wr1eqvPp/vKYjMAEZsRMDpdN6MRCKf+Xz+8cbGBm2kckIAtXShUGgKh8Mnfr9/Ybh0IKDT6a4vLS19EggE+wwGY7jmUj6TyUTQyv35+fmP0MbBexkISKVS+97e3u1LsU0oAo67Go3G1i/pCZjN5t3Nzc0H/zPzPlH/ip8EWvbQYrHs4hwWmN3e3n62traGfZy7MtbX12fYbPYTIKIQOzs7C6urq/cgINVWVlaO4EtjEXq9nken06mksgMZtJthtVp5xNzcHBsEIEWu0Wg0RKVS2USz2WTigFx6hPCkMTfRarXgB6WQzd/j63Q6bdK+nB7j0KnRaCCCIMpTE4D2oEql8m1qAvV6/VSr1U5PIJvN+qFjv6fyBIVC4ZfD4XgJAmekC9RqNZROp1+FQqESCCDSBWCVO1EoFC8wOQZpAnjm0Wj0O/xX94H4HNAzotvtNnreFU6lUgllMplAMpnclcvlp8NURC6Xi8BLGc5N9NvtNqpWq6hcLqN8Pn8OLUkAMe/g4EAOG4WzizcTJpPpA6zFIpfL9Ra2I45/IRgM2lKp1LtisXjs9XpvHR4eHimVythF4n78BwAA//8r8OPFAAAABklEQVQDAPCY8Oae0TJxAAAAAElFTkSuQmCC") 1 23, default`,he=String.raw`
(function () {
  "use strict";
  if (window.__coworkHtmlReviewGuest) return;
  window.__coworkHtmlReviewGuest = true;

  var READY_TYPE = "__cowork_html_review_ready__";
  var SEL_CLASS = "__cwhr-sel";
  var CONTEXT_LEN = 32;
  var EXACT_CAP = 1000;
  var port = null;
  var mode = false;
  var pins = [];
  var quotes = [];
  // Resolved Range per quote id; invalidated on DOM mutation and on a
  // fresh quotes set. Scroll/resize re-reports just re-read
  // getClientRects() from the cached Range. Null prototype - ids are
  // collaborator-writable and must not collide with Object.prototype.
  var quoteRanges = Object.create(null);
  var hoverEl = null;
  var selected = null;
  var hasTextSel = false;
  var rafPending = false;
  // Capture before page body scripts can shadow them; start() defers
  // to DOMContentLoaded which page code can race.
  var _MessageChannel = window.MessageChannel;
  var _topPost = window.top.postMessage.bind(window.top);

  function visibleText(el) {
    // innerText drops display:none/visibility:hidden subtrees; keep an
    // empty innerText (icon-only el) rather than falling through to
    // textContent which would re-include hidden text.
    var t = typeof el.innerText === "string" ? el.innerText : (el.textContent || "");
    return t.replace(/\s+/g, " ").trim();
  }

  // Comment-mode cursor: see HTML_PREVIEW_COMMENT_CURSOR above.
  var BUBBLE_CURSOR = ${JSON.stringify(k)};

  function injectStyle() {
    var style = document.createElement("style");
    // Hover box is painted by the host (from streamed rects); the guest
    // marks the selected element and, in comment mode, swaps the native
    // cursor for the comment bubble (forced, so page cursor styles don't
    // fight the picker). While a composer or thread card is open the arrow
    // returns, so the bubble cursor and the pin just placed never show at
    // once. Keyed on <html> data attributes, which page scripts that
    // rewrite body.className leave alone; ":not(#_)" lifts the rules to
    // id-level specificity so a page's own !important cursor rules lose.
    var ANY = ":not(#__cwhr_)";
    var MODE = "html[data-cwhr-mode]";
    var COMPOSING = MODE + "[data-cwhr-composing]";
    style.textContent =
      "." + SEL_CLASS + "{outline:1px solid hsl(40 70% 40%) !important;outline-offset:0;}" +
      MODE + ANY + "," + MODE + " " + ANY + "{cursor:" + BUBBLE_CURSOR + " !important;}" +
      COMPOSING + ANY + "," + COMPOSING + " " + ANY + "{cursor:default !important;}";
    (document.head || document.documentElement).appendChild(style);
  }

  function isUnique(sel) {
    try {
      return document.querySelectorAll(sel).length === 1;
    } catch (e) {
      return false;
    }
  }

  // Mirrors the host's stripInvisibles ranges. The host strips these
  // from stored selectors, so a selector built on a value containing
  // them would validate here but never match again - skip those rungs
  // and fall through to the structural ladder instead.
  function hasInvisibles(s) {
    for (var i = 0; i < s.length; i++) {
      var c = s.codePointAt(i);
      if (c > 0xffff) i++;
      if (
        c <= 0x1f ||
        c === 0x061c ||
        c === 0x070f ||
        c === 0xad ||
        (c >= 0x7f && c <= 0x9f) ||
        c === 0x115f ||
        c === 0x1160 ||
        c === 0x180e ||
        (c >= 0x200b && c <= 0x200f) ||
        (c >= 0x202a && c <= 0x202e) ||
        (c >= 0x2060 && c <= 0x2064) ||
        (c >= 0x2066 && c <= 0x2069) ||
        c === 0x3164 ||
        c === 0xfeff ||
        (c >= 0xfff9 && c <= 0xfffb) ||
        c === 0xffa0 ||
        (c >= 0xe0000 && c <= 0xe007f)
      ) {
        return true;
      }
    }
    return false;
  }

  function escapeAttr(v) {
    return String(v).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  }

  // Selector ladder: unique #id -> unique [data-*] -> structural
  // tag:nth-of-type path rooted at the nearest unique-id ancestor.
  function selectorFor(el) {
    if (el.id && !hasInvisibles(el.id) && isUnique("#" + CSS.escape(el.id))) {
      return "#" + CSS.escape(el.id);
    }
    for (var i = 0; i < el.attributes.length; i++) {
      var attr = el.attributes[i];
      if (
        attr.name.indexOf("data-") === 0 &&
        attr.value &&
        !hasInvisibles(attr.value)
      ) {
        var dataSel = "[" + attr.name + '="' + escapeAttr(attr.value) + '"]';
        if (isUnique(dataSel)) return dataSel;
      }
    }
    var parts = [];
    var cur = el;
    while (cur && cur !== document.body && cur.nodeType === 1 && parts.length < 10) {
      var sel = cur.tagName.toLowerCase();
      var parent = cur.parentElement;
      if (parent) {
        var sameTag = [];
        for (var c = 0; c < parent.children.length; c++) {
          if (parent.children[c].tagName === cur.tagName) sameTag.push(parent.children[c]);
        }
        if (sameTag.length > 1) {
          sel += ":nth-of-type(" + (sameTag.indexOf(cur) + 1) + ")";
        }
      }
      parts.unshift(sel);
      if (
        parent &&
        parent.id &&
        !hasInvisibles(parent.id) &&
        isUnique("#" + CSS.escape(parent.id))
      ) {
        parts.unshift("#" + CSS.escape(parent.id));
        return parts.join(" > ");
      }
      cur = parent;
    }
    if (cur === document.body) {
      parts.unshift("body");
      return parts.join(" > ");
    }
    // Depth cap hit without a root - an unrooted chain would match the
    // first structural pattern anywhere in the document. Mark the pin
    // detached instead of attaching it to the wrong element.
    return ":not(*)";
  }

  function descriptorFor(el) {
    var tag = "<" + el.tagName.toLowerCase() + ">";
    var idPart = el.id ? " #" + el.id : "";
    var text = visibleText(el).slice(0, 60);
    var textPart = text ? ' "' + text + '"' : "";
    // Nearest id'd ancestor, skipping the UCR-injected
    // #artifacts-component-root-* wrapper which is implementation noise.
    var section = el.parentElement;
    while (
      section &&
      (!section.id || section.id.indexOf("artifacts-component-root") === 0)
    ) {
      section = section.parentElement;
    }
    var sectionPart = section && section.id ? " in #" + section.id : "";
    return (tag + idPart + textPart + sectionPart).slice(0, 400);
  }

  // textContent offset of (node, offset) within body - the coordinate
  // system text-quote anchors are stored in. -1 when the boundary
  // isn't under body (shadow root, detached).
  function textOffset(node, offset) {
    var body = document.body;
    if (!body) return -1;
    try {
      var pre = document.createRange();
      pre.selectNodeContents(body);
      pre.setEnd(node, offset);
      return pre.toString().length;
    } catch (e) {
      return -1;
    }
  }

  // Inverse: build a Range covering [start, end) in body.textContent.
  function rangeForOffsets(start, end) {
    var body = document.body;
    if (!body) return null;
    var walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
    var acc = 0;
    var range = document.createRange();
    var haveStart = false;
    var t;
    while ((t = walker.nextNode())) {
      var next = acc + t.length;
      if (!haveStart && start < next) {
        range.setStart(t, start - acc);
        haveStart = true;
      }
      if (haveStart && end <= next) {
        range.setEnd(t, end - acc);
        return range;
      }
      acc = next;
    }
    return null;
  }

  // Context-scored search (W3C text-quote resolution): slice-check the
  // hint, else pick the occurrence whose surrounding chars best match
  // prefix/suffix. MIRRORS locateTextQuote() above line-for-line - the
  // tested export is the contract; this copy is what the sandbox runs.
  function locateQuote(text, q) {
    var ex = q.exact;
    if (!ex) return null;
    var hint = q.startOffset >= 0 ? q.startOffset : 0;
    if (text.slice(hint, hint + ex.length) === ex) {
      return { start: hint, end: hint + ex.length };
    }
    var best = -1;
    var bestScore = -1;
    var bestDist = Infinity;
    var scanned = 0;
    for (var idx = text.indexOf(ex); idx !== -1 && ++scanned <= 1000; idx = text.indexOf(ex, idx + 1)) {
      var before = text.slice(Math.max(0, idx - CONTEXT_LEN), idx);
      var after = text.slice(idx + ex.length, idx + ex.length + CONTEXT_LEN);
      var sc = 0;
      for (var i = 1; i <= q.prefix.length && i <= before.length; i++) {
        if (q.prefix[q.prefix.length - i] !== before[before.length - i]) break;
        sc++;
      }
      for (var j = 0; j < q.suffix.length && j < after.length; j++) {
        if (q.suffix[j] !== after[j]) break;
        sc++;
      }
      var dist = Math.abs(idx - hint);
      if (sc > bestScore || (sc === bestScore && dist < bestDist)) {
        best = idx;
        bestScore = sc;
        bestDist = dist;
      }
    }
    return best === -1 ? null : { start: best, end: best + ex.length };
  }

  function collectLineRects(range, cap) {
    var out = [];
    var rects = range.getClientRects();
    for (var i = 0; i < rects.length && out.length < cap; i++) {
      var r = rects[i];
      if (r.width > 0 && r.height > 0) {
        out.push({ x: r.left, y: r.top, w: r.width, h: r.height });
      }
    }
    return out;
  }

  function reportTextSel(full) {
    if (!port) return;
    var sel = window.getSelection();
    var r = sel && !sel.isCollapsed && sel.rangeCount > 0 ? sel.getRangeAt(0) : null;
    if (!r || !document.body || !document.body.contains(r.commonAncestorContainer)) {
      if (hasTextSel) {
        hasTextSel = false;
        port.postMessage({ type: "textClear" });
      }
      return;
    }
    hasTextSel = true;
    var br = r.getBoundingClientRect();
    var msg = {
      type: "textCapture",
      rect: { x: br.left, y: br.top, w: br.width, h: br.height },
      lineRects: collectLineRects(r, 80),
    };
    if (full) {
      var live = document.body.textContent || "";
      var so = textOffset(r.startContainer, r.startOffset);
      var eo = textOffset(r.endContainer, r.endOffset);
      if (so < 0 || eo < 0 || eo <= so) {
        // Unanchorable selection (shadow root, foreign node). A
        // rect-only post here would make the host pair the new rect
        // with a stale prior anchor - clear instead.
        hasTextSel = false;
        port.postMessage({ type: "textClear" });
        return;
      }
      // Denomination invariant: exact/prefix/suffix are byte-identical
      // slices of body.textContent. Truncation adjusts endOffset so
      // the host's (endOffset - startOffset === exact.length) shape
      // check holds. Cut points back off surrogate halves - a lone
      // surrogate would never relocate and fails UTF-8 persist.
      var capped = Math.min(eo, so + EXACT_CAP);
      if (capped < eo) {
        var hc = live.charCodeAt(capped - 1);
        if (hc >= 0xd800 && hc <= 0xdbff) capped--;
      }
      if (capped <= so) {
        port.postMessage(msg);
        return;
      }
      var pStart = Math.max(0, so - CONTEXT_LEN);
      var pc = live.charCodeAt(pStart);
      if (pc >= 0xdc00 && pc <= 0xdfff) pStart++;
      var sEnd = Math.min(live.length, capped + CONTEXT_LEN);
      if (sEnd < live.length) {
        var sc = live.charCodeAt(sEnd - 1);
        if (sc >= 0xd800 && sc <= 0xdbff) sEnd--;
      }
      msg.exact = live.slice(so, capped);
      msg.prefix = live.slice(pStart, so);
      msg.suffix = live.slice(capped, sEnd);
      msg.startOffset = so;
      msg.endOffset = capped;
    }
    port.postMessage(msg);
  }

  function clearSelection() {
    if (selected) {
      selected.classList.remove(SEL_CLASS);
      selected = null;
    }
  }

  // rAF-throttled cursor + hover-rect stream so the host can paint the
  // inspector hover box (and knows the pointer is present) in comment mode.
  var cursorPending = false;
  var cursorX = -1;
  var cursorY = -1;
  function flushCursor() {
    cursorPending = false;
    if (!port) return;
    var hover = null;
    if (
      hoverEl &&
      hoverEl.nodeType === 1 &&
      hoverEl !== document.body &&
      hoverEl !== document.documentElement
    ) {
      var r = hoverEl.getBoundingClientRect();
      if (r.width > 0 || r.height > 0) {
        hover = { x: r.left, y: r.top, w: r.width, h: r.height };
      }
    }
    port.postMessage({ type: "cursor", x: cursorX, y: cursorY, hover: hover });
  }
  function scheduleCursor() {
    if (!cursorPending) {
      cursorPending = true;
      requestAnimationFrame(flushCursor);
    }
  }
  function onMouseMove(ev) {
    if (!mode || !port) return;
    // Over the scrollbar gutter (clientX/Y past the content box) - hide
    // the hover box like a leave.
    var de = document.documentElement;
    if (ev.clientX >= de.clientWidth || ev.clientY >= de.clientHeight) {
      cursorX = -1;
      cursorY = -1;
      hoverEl = null;
      scheduleCursor();
      return;
    }
    cursorX = ev.clientX;
    cursorY = ev.clientY;
    hoverEl = ev.target;
    scheduleCursor();
  }
  function onMouseLeave() {
    if (!mode || !port) return;
    cursorX = -1;
    cursorY = -1;
    hoverEl = null;
    scheduleCursor();
  }

  function onClick(ev) {
    if (!port) return;
    if (!mode) {
      // Mode off: don't intercept the page, just let the host close any
      // open popover (thread card click-away). A non-collapsed selection
      // means the click ended a drag-select - not a click-away.
      var s = window.getSelection();
      if (!s || s.isCollapsed) port.postMessage({ type: "clickAway" });
      return;
    }
    ev.preventDefault();
    ev.stopImmediatePropagation();
    var el = ev.target;
    if (!el || el.nodeType !== 1 || el === document.documentElement) return;
    if (el === document.body && el.firstElementChild) {
      // Pin on body is rarely intentional; keep it anyway but as body.
    }
    clearSelection();
    selected = el;
    el.classList.add(SEL_CLASS);
    var rect = el.getBoundingClientRect();
    var pinX = rect.width > 0 ? (ev.clientX - rect.left) / rect.width : 0.5;
    var pinY = rect.height > 0 ? (ev.clientY - rect.top) / rect.height : 0.5;
    port.postMessage({
      type: "capture",
      selector: selectorFor(el).slice(0, 1000),
      textHint: visibleText(el).slice(0, 300),
      descriptor: descriptorFor(el),
      pinX: Math.min(1, Math.max(0, pinX)),
      pinY: Math.min(1, Math.max(0, pinY)),
      clientX: ev.clientX,
      clientY: ev.clientY,
      rectLeft: rect.left,
      rectTop: rect.top,
    });
  }

  function reportRects() {
    rafPending = false;
    if (!port) return;
    // Selected element's live bounds - keeps the host's draft pin /
    // inspector tag / composer glued through scroll and reflow.
    if (selected) {
      var sr = selected.getBoundingClientRect();
      port.postMessage({
        type: "selRect",
        left: sr.left,
        top: sr.top,
        w: sr.width,
        h: sr.height,
        found:
          (sr.width > 0 || sr.height > 0) &&
          document.documentElement.contains(selected),
      });
    }
    var rects = [];
    for (var i = 0; i < pins.length; i++) {
      var p = pins[i];
      var el = null;
      try {
        el = document.querySelector(p.selector);
      } catch (e) {
        el = null;
      }
      if (!el) {
        rects.push({ id: p.id, x: 0, y: 0, found: false });
        continue;
      }
      var r = el.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) {
        rects.push({ id: p.id, x: 0, y: 0, found: false });
        continue;
      }
      rects.push({
        id: p.id,
        x: r.left + p.pinX * r.width,
        y: r.top + p.pinY * r.height,
        found: true,
      });
    }
    var quoteRects = [];
    if (quotes.length > 0) {
      var live = null;
      for (var qi = 0; qi < quotes.length; qi++) {
        var q = quotes[qi];
        var qr = quoteRanges[q.id];
        if (qr === undefined) {
          if (live === null) {
            live = (document.body && document.body.textContent) || "";
          }
          var loc = locateQuote(live, q);
          qr = (loc && rangeForOffsets(loc.start, loc.end)) || null;
          quoteRanges[q.id] = qr;
        }
        // Zero paintable rects (display:none subtree) reports as
        // not-found so the host docks it as detached instead of
        // rendering nothing clickable.
        var qlr = qr ? collectLineRects(qr, 80) : [];
        quoteRects.push(
          qlr.length > 0
            ? { id: q.id, found: true, lineRects: qlr }
            : { id: q.id, found: false, lineRects: [] }
        );
      }
    }
    port.postMessage({
      type: "rects",
      rects: rects,
      quoteRects: quoteRects,
      scrollY: window.scrollY || 0,
    });
  }

  function scheduleRects() {
    if (rafPending) return;
    rafPending = true;
    var done = false;
    function run() {
      if (done) return;
      done = true;
      reportRects();
    }
    // rAF for paint alignment, with a timeout fallback - rAF is
    // suspended entirely in hidden tabs and pins must still settle
    // when the tab becomes visible after background loading.
    requestAnimationFrame(run);
    setTimeout(run, 150);
  }

  function setMode(on) {
    mode = !!on;
    document.documentElement.toggleAttribute("data-cwhr-mode", mode);
    if (!mode) {
      hoverEl = null;
      clearSelection();
    }
  }

  function setComposing(open) {
    document.documentElement.toggleAttribute("data-cwhr-composing", !!open);
  }

  function onPortMessage(ev) {
    var data = ev.data;
    if (!data) return;
    if (data.type === "mode") setMode(data.on);
    else if (data.type === "composing") setComposing(data.open);
    else if (data.type === "pins") {
      pins = Array.isArray(data.pins) ? data.pins : [];
      scheduleRects();
    } else if (data.type === "quotes") {
      quotes = Array.isArray(data.quotes) ? data.quotes : [];
      quoteRanges = Object.create(null);
      scheduleRects();
    } else if (data.type === "clearSelection") {
      clearSelection();
    } else if (data.type === "clearTextSel") {
      var ts = window.getSelection();
      if (ts) ts.removeAllRanges();
      hasTextSel = false;
    } else if (data.type === "restoreScroll") {
      var ry = Number(data.y);
      if (isFinite(ry) && ry > 0) {
        // Apply now and again at load - late images/fonts can grow the
        // document after the first attempt clamps short.
        window.scrollTo(0, ry);
        window.addEventListener("load", function () {
          window.scrollTo(0, ry);
        });
      }
    }
  }

  function start() {
    injectStyle();
    var channel = new _MessageChannel();
    port = channel.port1;
    port.onmessage = onPortMessage;
    // Sandboxed srcdoc origins can be opaque; "*" is required. The
    // payload carries only the mount token (multiple sandboxes can be
    // mounted at once - the token routes the port to the right overlay)
    // and the channel port itself is pairwise-secure.
    _topPost(
      { type: READY_TYPE, token: "__CWHR_TOKEN__" },
      "*",
      [channel.port2]
    );
    document.addEventListener("mousemove", onMouseMove, true);
    // documentElement (not document, capture) so this fires only when the
    // pointer leaves the viewport - capture-phase on document would fire for
    // every intra-document element-boundary crossing.
    document.documentElement.addEventListener("mouseleave", onMouseLeave);
    document.addEventListener("click", onClick, true);
    // Text-selection capture: commit on mouseup/keyup (Shift+Arrow),
    // clear on collapse. Not mode-gated; highlight-to-comment is
    // always on.
    function onSelCommit() { reportTextSel(true); }
    document.addEventListener("mouseup", onSelCommit, true);
    document.addEventListener("keyup", onSelCommit, true);
    document.addEventListener("selectionchange", function () {
      var s = window.getSelection();
      if (hasTextSel && (!s || s.isCollapsed)) reportTextSel(false);
    });
    document.addEventListener("keydown", function (ev) {
      if (ev.key === "Escape" && port) port.postMessage({ type: "dismiss" });
    }, true);
    // Scroll/resize move the hovered element under a stationary pointer -
    // re-report its rect so the host's inspector box tracks it. Mutations
    // stay rects-only (animating pages would chatter the cursor stream).
    var textSelPending = false;
    function scheduleTextSel() {
      if (textSelPending) return;
      textSelPending = true;
      requestAnimationFrame(function () {
        textSelPending = false;
        reportTextSel(false);
      });
    }
    function onLayoutChange() {
      scheduleRects();
      if (mode && hoverEl) scheduleCursor();
      if (hasTextSel) scheduleTextSel();
    }
    window.addEventListener("scroll", onLayoutChange, true);
    window.addEventListener("resize", onLayoutChange);
    // Full-cache invalidate per childList mutation is acceptable: CSS
    // animation doesn't fire the observer; JS writers at frame rate
    // would re-relocate every frame but are rare. characterData isn't
    // observed, so .data= edits leave highlights stale until the next
    // childList mutation.
    var observer = new MutationObserver(function () {
      quoteRanges = Object.create(null);
      scheduleRects();
      if (hasTextSel) scheduleTextSel();
    });
    function observe() {
      if (document.body) observer.observe(document.body, { childList: true, subtree: true });
    }
    if (document.body) observe();
    else document.addEventListener("DOMContentLoaded", observe);
    // Settle pins after late layout (fonts, images).
    setInterval(scheduleRects, 2000);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
`;function ge(e,t){let n=t.replace(/[^a-zA-Z0-9-]/g,""),r=`<script>${he.replace("__CWHR_TOKEN__",n)}<\/script>`,i=e=>" ".repeat(e.length),a=e.replace(/<!--[\s\S]*?-->/g,i).replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi,i).replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi,i).replace(/<(?:script|style)\b[^>]*>[\s\S]*$/i,i),o=/<head(?:\s[^>]*)?>/i.exec(a);if(o){let t=o.index+o[0].length;return e.slice(0,t)+r+e.slice(t)}let s=/<html(?:\s[^>]*)?>/i.exec(a);if(s){let t=s.index+s[0].length;return e.slice(0,t)+r+e.slice(t)}let c=/^\s*<!doctype[^>]*>/i.exec(a);if(c){let t=c.index+c[0].length;return e.slice(0,t)+r+e.slice(t)}return r+e}var _e=24,ve="#D97757",A="#C6613F",ye="rgba(0,0,0,0.12)",be="0 1px 3px rgba(0,0,0,0.10), 0 4px 10px rgba(0,0,0,0.08)",xe="0 0 0 0.5px rgba(0,0,0,0.06), 0 4px 12px rgba(0,0,0,0.06), 0 16px 40px rgba(0,0,0,0.08)",Se={initial:{opacity:0,y:6,scale:.98},animate:{opacity:1,y:0,scale:1},transition:{duration:.16,ease:[.2,.7,.3,1]}},Ce="rgba(253, 186, 116, 0.35)",we="rgba(253, 186, 116, 0.55)",Te=80,Ee=200,j=256,De=84,Oe=120,ke=168,M=6;function Ae(e){let t=N(e.x+M,M,Math.max(M,e.boundsW-j-M)),n=e.y+M;return{left:t,top:n+e.estH<=e.boundsH-M?n:N(e.y-_e-M-e.estH,M,Math.max(M,e.boundsH-e.estH-M))}}function N(e,t,n){return Math.min(n,Math.max(t,e))}function je(e,t,n){return`min(${N(e||0,t,1e6)}px, max(${t}px, 100% - ${n}px))`}function Me(e,t){let n="var(--cds-h-control)",r=`calc(${n} / 2 + 0.25rem + 4px)`,i=`calc(${n} + 0.5rem + 4px)`,a=e=>`${N(e||0,0,1e6)}px`;return{left:`clamp(${r}, ${a(e)}, calc(100% - ${r}))`,top:`clamp(${i}, ${a(t)}, calc(100% - 4px))`}}function Ne(e,t){let n=Number(e?.x)||0,r=Number(e?.y)||0,i=N(n,0,t.w),a=N(r,0,t.h),o=N(n+(Number(e?.w)||0),0,t.w),s=N(r+(Number(e?.h)||0),0,t.h);return o>i&&s>a?{x:i,y:a,w:o-i,h:s-a}:null}function Pe(e,t){if(!Array.isArray(e))return[];let n=[];for(let r of e.slice(0,Te)){let e=Ne(r,t);e&&n.push(e)}return n}function Fe(e,t){return typeof e=="string"?D(e).slice(0,t):""}function Ie(e){return e.toUpperCase().toLowerCase().replace(/\u03C2/g,"\u03C3").replace(/\s+/g,"")}function P(t){try{return oe(Le(t),2e6)}catch(t){let n=e(Error("HTML preview text index failed"),"HTML preview text index failed","literal");return y(t instanceof Error?b(t,n):n,{source:"html_preview_text_index",error_name:t instanceof Error?t.name:typeof t}),""}}function Le(e){let t=new DOMParser().parseFromString(e,"text/html"),n=e=>Document.prototype.querySelectorAll.call(t,e);for(let e of n("script, style, noscript, template, [hidden], dialog:not([open]), details:not([open]) > :not(summary)"))Element.prototype.remove.call(e);for(let e of n("[style]")){let t=Element.prototype.getAttribute.call(e,"style")??"",n=Reflect.get(Object.getPrototypeOf(e),"style",e);(n.display.toLowerCase()==="none"||/^(?:hidden|collapse)$/.test(n.visibility.toLowerCase())||n.getPropertyValue("content-visibility").toLowerCase()==="hidden"||/(?:^|;)\s*display\s*:\s*none\b/i.test(t)||/(?:^|;)\s*(?:content-)?visibility\s*:\s*(?:hidden|collapse)\b/i.test(t))&&Element.prototype.remove.call(e)}let r=Reflect.get(Document.prototype,"body",t),i=Document.prototype.createTreeWalker.call(t,r??t,NodeFilter.SHOW_TEXT),a="";for(;i.nextNode();)a+=(i.currentNode.textContent??"")+" ";return a}var Re=String.raw`#(?:[-\w\u0080-\uFFFF]|\\[0-9a-fA-F]{1,6} |\\[^0-9a-fA-F])+`,ze=String.raw`\[data-[a-zA-Z0-9_.:-]+="(?:[^"\\]|\\.)*"\]`,F=String.raw`[a-z][a-z0-9-]*(?::nth-of-type\(\d{1,6}\))?`,Be=RegExp(`^(?:${Re}|${ze}|:not\\(\\*\\)|(?:(?:${Re}|body) > )?${F}(?: > ${F}){0,9})$`),I=/"(?:[^"\\]|\\.){81,}"/,Ve=/"[^"]*\s[^"]*"/,He=new RegExp(String.raw`#(?:[-\w\u0080-\uFFFF]|\\[0-9a-fA-F]{1,6} |\\[^0-9a-fA-F]){65,}`),Ue=/(?:^|> )[a-z][a-z0-9-]{32,}/,We=/\[data-[a-zA-Z0-9_.:-]{33,}=/,Ge=new RegExp(String.raw`^#(?:[-\w\u0080-\uFFFF]|\\[0-9a-fA-F]{1,6} |\\[^0-9a-fA-F]){33,} > `),Ke=20,qe=new Set("a abbr address area article aside audio b base bdi bdo blockquote body br button canvas caption cite code col colgroup data datalist dd del details dfn dialog div dl dt em embed fieldset figcaption figure footer form h1 h2 h3 h4 h5 h6 head header hgroup hr html i iframe img input ins kbd label legend li link main map mark menu meta meter nav noscript object ol optgroup option output p picture pre progress q rp rt ruby s samp script search section select slot small source span strong style sub summary sup table tbody td template textarea tfoot th thead time title tr track u ul var video wbr svg math g path rect circle line text tspan polyline polygon ellipse defs use symbol image foreignobject".split(" ")),Je=/^#artifacts-component-root-(?:html|react|svg|pdf)(?: > |$)/;function Ye(e){let t=globalThis.CSS?.escape;if(t)return t(e);let n="";for(let t=0;t<e.length;t++){let r=e.charCodeAt(t);r===0?n+="\uFFFD":r>=1&&r<=31||r===127||t===0&&r>=48&&r<=57||t===1&&r>=48&&r<=57&&e.charCodeAt(0)===45?n+=`\\${r.toString(16)} `:t===0&&e.length===1&&r===45?n+="\\-":n+=r>=128||r===45||r===95||r>=48&&r<=57||r>=65&&r<=90||r>=97&&r<=122?e[t]:`\\${e[t]}`}return n}function Xe(e){return I.test(e)||Ve.test(e)||He.test(e)||Ue.test(e)||We.test(e)||Ge.test(e)}var Ze=/^<([a-z][\w-]{0,31})>(?: #([^\s"]{1,64}))?(?: "([^"]{0,60})")?(?: in #(\S{1,64}))?$/,L=/^artifacts-component-root/,Qe=c(function(e){let $=J9(19),{descriptor:t}=e,n,i,o,s,c;if(J1($,0,t)){o=Symbol.for("react.early_return_sentinel");bb0:{let e=Ze.exec(t);if(!e){o=r("div",{className:"truncate text-xs text-muted-as-secondary",children:t});break bb0}let[,a,l,u,d]=e;s=a,n=l,c=u,i=d&&!L.test(d)?d:void 0}K5($,0,t,n,i,o,s,c)}else n=$[1],i=$[2],o=$[3],s=$[4],c=$[5];if(o!==Symbol.for("react.early_return_sentinel"))return o;let l=i,u=J1($,6,s)?K1($,6,s,r("span",{className:"inline-flex shrink-0 items-center rounded border bg-surface-1 px-1 font-mono text-[11px] text-secondary",children:s})):$[7],d=J2($,8,u,c)?K2($,8,u,c,c?r(m,{multiline:!0,content:r("span",{className:"line-clamp-3 break-words",children:`\u201C${c}\u201D`}),children:u}):u):$[10],f=J1($,11,n)?K1($,11,n,n&&r("span",{className:"shrink-0 font-mono text-accent-pro-100",children:`#${n}`})):$[12],p=J1($,13,l)?K1($,13,l,l&&r("span",{className:"shrink-0 truncate",children:r(re,{defaultMessage:"in <s>#{section}</s>",id:"6XJT5DKRek",values:{section:l,s:lt}})})):$[14];return J3($,15,d,f,p)?K3($,15,d,f,p,a("div",{className:"flex min-w-0 select-none items-center gap-1.5 text-xs text-muted-as-secondary",children:[d,f,p]})):$[18]});function $e(e,t,n){let r=Ze.exec(e.descriptor),i=r?.[1]??"",a=r?.[2]??"",o=r?.[4]??"",s=e.selector,c=s.replace(Je,""),l=!t.descriptor&&a&&s!==`#${Ye(a)}`?`#${a} \u00B7 `:"",u=!t.descriptor&&o&&!s.startsWith(`#${Ye(o)} > `)?`in #${o} \u00B7 `:"",d=`${l}${u}${t.selector?s:c}`,f=`${l}${u}${s}`,p=c===""||c.startsWith("[")?[]:c.replace(/:nth-of-type\(\d+\)/g,"").split(" > "),m=p[0]?.startsWith("#")?p[0].slice(1):"",h=(m?p.slice(1):p).filter(e=>e!==":not(*)");return{tag:i,idPart:l,sectionPart:u,selectorLine:d,rawSelectorLine:f,show:d!==""&&(t.selector||n&&(m!==""||l!==""||u!=="")||c.startsWith("[")||h.some(e=>!qe.has(e))||/[\\]|[^ -~]/.test(s+a+o)||[m,a,o].some(e=>e.length>Ke))}}function et(e,t){switch(e){case"p":return t.formatMessage({defaultMessage:"Paragraph",id:"qKqtMCzfla"});case"h1":case"h2":case"h3":case"h4":case"h5":case"h6":return t.formatMessage({defaultMessage:"Heading",id:"jT6/daUxjc"});case"button":return t.formatMessage({defaultMessage:"Button",id:"KP63fg1AMS"});case"a":return t.formatMessage({defaultMessage:"Link",id:"thhrQH6sOO"});case"img":return t.formatMessage({defaultMessage:"Image",id:"+0zv6gS/c6"});case"li":return t.formatMessage({defaultMessage:"List item",id:"8zKBQnvxAY"});case"ul":case"ol":return t.formatMessage({defaultMessage:"List",id:"clX+Jd5gHY"});case"table":return t.formatMessage({defaultMessage:"Table",id:"IeBejt65iX"});case"td":case"th":return t.formatMessage({defaultMessage:"Cell",id:"/jd9eE01Ei"});case"input":case"textarea":case"select":return t.formatMessage({defaultMessage:"Field",id:"AbvQFbGv8I"});case"main":return t.formatMessage({defaultMessage:"Main",id:"tTDTZgoxfQ"});case"section":return t.formatMessage({defaultMessage:"Section",id:"q1Cd7mwNKK"});case"header":return t.formatMessage({defaultMessage:"Header",id:"vUjsX7qaCx"});case"footer":return t.formatMessage({defaultMessage:"Footer",id:"Vge+RXo3fP"});case"nav":return t.formatMessage({defaultMessage:"Navigation",id:"fBg+7V6ZhY"});case"aside":return t.formatMessage({defaultMessage:"Sidebar",id:"JZy6joR+Wk"});case"article":return t.formatMessage({defaultMessage:"Article",id:"jx7Hn3QkH8"});case"form":return t.formatMessage({defaultMessage:"Form",id:"baRFiFaNBT"});case"label":return t.formatMessage({defaultMessage:"Label",id:"YcWAGM4CXv"});case"span":return t.formatMessage({defaultMessage:"Text",id:"aA8bDwsPeJ"});case"div":return t.formatMessage({defaultMessage:"Block",id:"S/5TeMMpVn"});case"body":return t.formatMessage({defaultMessage:"Page",id:"wFmAFCPAci"});default:return null}}var R=16,tt=c(function(e){let $=J9(14),{descriptor:t,x:n,y:i}=e,o=J1($,0,t)?K1($,0,t,Ze.exec(t)):$[1],s=J1($,2,t)||J1($,3,o?.[1])?I1($,3,Z1($,2,o?.[1]??t.slice(0,24),t),o?.[1]):$[4],c=o?.[2],l=i<R,u=n-1,d=l?i:i-R,f=J2($,5,u,d)?K2($,5,u,d,{left:u,top:d,backgroundColor:ve,color:"#fff"}):$[7],p=J1($,8,c)?K1($,8,c,c&&r("span",{style:{opacity:.8},children:` #${c}`})):$[9];return J3($,10,f,p,s)?K3($,10,f,p,s,a("div",{className:"pointer-events-none absolute z-20 max-w-[16rem] truncate px-1 py-px font-mono text-[10px] leading-[14px]",style:f,children:[s,p]})):$[13]}),nt=c(function(e){let $=J9(17),{n:t,active:n,swingIn:i}=e,a=i!==void 0&&i,o=C()??!1,c=J0($,0)?K0($,0,{stiffness:300,damping:22}):$[0],l=T(!o&&(n||a)||n?-90:0,c),u,d;J3($,1,n,o,l)?(u=()=>{o?l.jump(n?-90:0):l.set(n?-90:0)},d=[n,l,o],K4($,1,n,o,l,u,d)):(u=$[4],d=$[5]),s(u,d);let f=w(l,ut),p=n?ve:"#fff",m=`0 0 0 1px ${n?A:ye}, ${be}`,h=J3($,6,l,p,m)?K3($,6,l,p,m,{rotate:l,transformOrigin:"1px 23px",backgroundColor:p,boxShadow:m}):$[9],g=J3($,10,n,f,t)?K3($,10,n,f,t,t!==void 0&&r(ee.span,{className:"text-[11px] font-semibold leading-none",style:{rotate:f,color:n?"#fff":"#141413"},children:t})):$[13];return J2($,14,h,g)?K2($,14,h,g,r(ee.div,{className:"flex size-6 items-center justify-center rounded-full rounded-bl-[2px]",style:h,children:g})):$[16]});function rt(e,t){if(e.length!==t.length)return!1;for(let n=0;n<e.length;n++){let r=e[n],i=t[n];if(r.x!==i.x||r.y!==i.y||r.w!==i.w||r.h!==i.h)return!1}return!0}function it(e,t){if(e.size!==t.size)return!1;for(let[n,r]of t){let t=e.get(n);if(!t||t.found!==r.found||!rt(t.lineRects,r.lineRects))return!1}return!0}function at(e,t){if(!e||e.size!==t.size)return!1;for(let[n,r]of t){let t=e.get(n);if(!t||t.x!==r.x||t.y!==r.y||t.found!==r.found)return!1}return!0}var ot=[],st=c(function(e){let $=J9(287),{active:c,token:p,documentRef:g,commentProps:_,uiHidden:v,commentMode:y,onCommentModeChange:b,pageTextIndex:S,host:C}=e,w=v!==void 0&&v,T=ie(),D=i()==="rtl"?"auto":void 0,{userContentRendererUrl:de}=x(),{containerRef:O,setKeyDownHandler:k}=C,[he,ge]=l(!1),[A,Ce]=l(null),[we,Te]=l(null),j=ne(0),P=ne(0),Le=b!==void 0,[Re,ze]=l(!1),F=Le?y??!1:Re,I=J1($,0,b)?K1($,0,b,e=>{b?b(e):ze(e)}):$[1],Ve=n(!1),He,Ue;J1($,2,F)?(He=()=>{Ve.current=F},Ue=[F],K2($,2,F,He,Ue)):(He=$[3],Ue=$[4]),s(He,Ue);let We,Ge;J1($,5,I)?(We=()=>{Tt.current=I},Ge=[I],K2($,5,I,We,Ge)):(We=$[6],Ge=$[7]),s(We,Ge);let Ke=n(null),Je,Ye;J1($,8,O)?(Je=()=>{let e=O.current;if(!e)return;let t=new ResizeObserver(e=>{let[t]=e;Ke.current={w:t.contentRect.width,h:t.contentRect.height}});return t.observe(e),()=>t.disconnect()},Ye=[O],K2($,8,O,Je,Ye)):(Je=$[9],Ye=$[10]),s(Je,Ye);let[L,Qe]=l(null),R=n(null),st,lt;J1($,11,L)?(lt=()=>{R.current=L},st=[L],K2($,11,L,st,lt)):(st=$[12],lt=$[13]),s(lt,st);let ut=n(0),St=n(void 0),Ct,wt;J1($,14,S)?(Ct=()=>{St.current=S===void 0?void 0:Ie(S)},wt=[S],K2($,14,S,Ct,wt)):(Ct=$[15],wt=$[16]),s(Ct,wt);let Tt=n(null),[z,Et]=l(null),[B,Dt]=l(null),[V,Ot]=l(null),[H,U]=l(null),kt=n(!1),At=n(null),jt,Mt;J1($,17,H)?(jt=()=>{kt.current=H!==null,At.current=H?.kind??null},Mt=[H],K2($,17,H,jt,Mt)):(jt=$[18],Mt=$[19]),s(jt,Mt);let[W,Nt]=l(""),Pt=n(null),{dir:Ft}=fe(Pt,W),[G,It]=l(!1),[K,q]=l(null),[J,Lt]=l(null),Rt=n(0),zt=n(!1),Bt,Vt;J1($,20,K)?(Bt=()=>{zt.current=K!==null},Vt=[K],K2($,20,K,Bt,Vt)):(Bt=$[21],Vt=$[22]),s(Bt,Vt);let Y=J1($,23,g)?K1($,23,g,se(g)):$[24],Ht=g.kind==="file",Ut=n(Ht),Wt,Gt;J1($,25,Ht)?(Wt=()=>{Ut.current=Ht},Gt=[Ht],K2($,25,Ht,Wt,Gt)):(Wt=$[26],Gt=$[27]),s(Wt,Gt);let Kt=_?.comments??ot,qt;if(J2($,28,Kt,Y)){let e=J1($,31,Y)?K1($,31,Y,e=>ce(e)&&(e.status??"open")==="open"&&e.target.anchor.kind==="element"&&se(e.target.document)===Y):$[32];qt=Kt.filter(e),K2($,28,Kt,Y,qt)}else qt=$[30];let X=qt,Jt=J1($,33,Kt)?K1($,33,Kt,ue(Kt)):$[34],Yt=J1($,35,X)?K1($,35,X,X.map(dt)):$[36],Xt=J0($,37)?K0($,37,new Set):$[37],Zt=n(Xt),Qt,$t;J1($,38,Yt)?(Qt=()=>{Zt.current=new Set(Yt.map(ft))},$t=[Yt],K2($,38,Yt,Qt,$t)):(Qt=$[39],$t=$[40]),s(Qt,$t);let en;if(J2($,41,Kt,Y)){let e=J1($,44,Y)?K1($,44,Y,e=>ce(e)&&(e.status??"open")==="open"&&e.target.anchor.kind==="text-quote"&&e.id!=="__cwhr_draft"&&se(e.target.document)===Y):$[45];en=Kt.filter(e).slice(0,Ee),K2($,41,Kt,Y,en)}else en=$[43];let Z=en,tn=J1($,46,Z)?K1($,46,Z,Z.map(pt)):$[47],nn=H?.kind==="text"?H.anchor:null,rn=J2($,48,tn,nn)?K2($,48,tn,nn,nn?[...tn,{id:pe,exact:nn.exact,prefix:nn.prefix,suffix:nn.suffix,startOffset:nn.startOffset}]:tn):$[50],an=J0($,51)?K0($,51,new Set):$[51],on=n(an),sn,cn;J1($,52,rn)?(sn=()=>{on.current=new Set(rn.map(mt))},cn=[rn],K2($,52,rn,sn,cn)):(sn=$[53],cn=$[54]),s(sn,cn);let ln=J2($,55,X,Z)?K2($,55,X,Z,[...X,...Z]):$[57],un=J0($,58)?K0($,58,e=>{let t=e;if(!(!t||typeof t!="object")){if(t.type==="capture"){if(!Ve.current)return;if(kt.current||zt.current){R.current?.postMessage({type:"clearSelection"}),U(ht),q(null);return}let e=Ke.current?.w??600,n=Ke.current?.h??400,r=Fe(t.selector,1e3);if(!Be.test(r)){R.current?.postMessage({type:"clearSelection"});return}let i=oe(t.textHint,300),a=oe(t.descriptor,400),o=i!==""&&(St.current===void 0?i.length>60:!St.current.includes(Ie(i))),s=Ze.exec(a)?.[3]??"",c={text:o,descriptor:s!==""&&!i.startsWith(s)||a!==""&&!Ze.test(a),selector:Xe(r)};U({kind:"element",captureId:Rt.current+=1,x:N(Number(t.clientX)||0,0,e),y:N(Number(t.clientY)||0,0,n),boundsW:e,boundsH:n,labelX:N(Number(t.rectLeft)||0,0,e),labelY:N(Number(t.rectTop)||0,0,n),anchor:{selector:r,textHint:i,descriptor:a,pinX:N(Number(t.pinX)||0,0,1),pinY:N(Number(t.pinY)||0,0,1)},suspect:c}),Nt(""),It(c.text||c.descriptor||c.selector),q(null),Ce(null),Te(null)}else if(t.type==="rects"){if(!Array.isArray(t.rects))return;let e=Number(t.scrollY);if(Number.isFinite(e)&&e>=0&&(ut.current=e),Et(e=>{let n=Zt.current,r=Math.max(n.size*2,16),i=t.rects.length>r?t.rects.slice(0,r):t.rects,a=new Map;for(let e of i)typeof e?.id!="string"||!n.has(e.id)||a.set(e.id,{id:e.id,x:Number(e.x)||0,y:Number(e.y)||0,found:e.found===!0});return at(e,a)?e:a}),Array.isArray(t.quoteRects)){let e=on.current,n=Ke.current??{w:600,h:400},r=t.quoteRects.slice(0,Math.max(e.size*2,16));Dt(t=>{let i=new Map;for(let t of r)typeof t?.id!="string"||!e.has(t.id)||i.set(t.id,{id:t.id,found:t.found===!0,lineRects:Pe(t.lineRects,n)});return t&&it(t,i)?t:i})}}else if(t.type==="dismiss"){if(zt.current&&q(null),!Ve.current)return;kt.current&&At.current==="element"?(R.current?.postMessage({type:"clearSelection"}),U(null)):zt.current||Tt.current?.(!1)}else if(t.type==="clickAway")q(null);else if(t.type==="selRect"){if(!kt.current||t.found!==!0)return;let e=Number(t.left),n=Number(t.top),r=Number(t.w),i=Number(t.h);if(!Number.isFinite(e)||!Number.isFinite(n)||!Number.isFinite(r)||!Number.isFinite(i))return;let a=Ke.current??{w:600,h:400};U(t=>{if(t?.kind!=="element")return t;let o=N(e+t.anchor.pinX*r,0,a.w),s=N(n+t.anchor.pinY*i,0,a.h);return o===t.x&&s===t.y?t:{...t,x:o,y:s,labelX:N(e,0,a.w),labelY:N(n,0,a.h)}})}else if(t.type==="cursor"){if(!Ve.current)return;let e=Number(t.x),n=Number(t.y);if(!Number.isFinite(e)||!Number.isFinite(n))return;let r=Ke.current??{w:600,h:400};if(e<0||n<0||e>r.w||n>r.h){ge(!1),Ce(null);return}ge(!0);let i=t.hover;Ce(e=>{if(!i||typeof i!="object")return e===null?e:null;let t={x:N(Number(i.x)||0,0,r.w),y:N(Number(i.y)||0,0,r.h),w:N(Number(i.w)||0,0,r.w),h:N(Number(i.h)||0,0,r.h)};return e&&e.x===t.x&&e.y===t.y&&e.w===t.w&&e.h===t.h?e:t})}else if(t.type==="textCapture"){if(!Ut.current)return;let e=Ke.current??{w:600,h:400},n=Ne(t.rect,e)??{x:0,y:0,w:0,h:0},r=Pe(t.lineRects,e),i=null;if(typeof t.exact=="string"){let e=E(t.exact.slice(0,me)),n=Math.max(0,Math.trunc(Number(t.startOffset))||0),r=Math.max(0,Math.trunc(Number(t.endOffset))||0);e.length>0&&r-n===e.length&&(i={exact:e,prefix:E(typeof t.prefix=="string"?t.prefix.slice(-32):""),suffix:E(typeof t.suffix=="string"?t.suffix.slice(0,32):""),startOffset:n,endOffset:r})}Ot(e=>{let t=i??e?.anchor??null;return t?e&&e.anchor.exact===t.exact&&e.anchor.prefix===t.prefix&&e.anchor.suffix===t.suffix&&e.anchor.startOffset===t.startOffset&&e.anchor.endOffset===t.endOffset&&e.rect.x===n.x&&e.rect.y===n.y&&e.rect.w===n.w&&e.rect.h===n.h&&rt(e.lineRects,r)?e:{rect:n,lineRects:r,anchor:t}:e})}else t.type==="textClear"&&Ot(null)}}):$[58],dn=n(un),fn,pn;$[59]===J8?(fn=()=>{dn.current=un},pn=[un],$[59]=fn,$[60]=pn):(fn=$[59],pn=$[60]),s(fn,pn);let mn,hn;J2($,61,p,de)?(mn=()=>{if(!p)return;let e=null;try{e=new URL(de).origin}catch{return}let t=t=>{let n=t.source===window&&t.origin===window.location.origin;if(t.origin!==e&&!n)return;let r=t.data;if(!r||r.type!=="__cowork_html_review_ready__"||r.token!==p)return;let i=t.ports[0];i&&(i.onmessage=e=>dn.current(e.data),Qe(i),Et(null),Dt(null),Ot(null))};return window.addEventListener("message",t),()=>{window.removeEventListener("message",t)}},hn=[p,de],K3($,61,p,de,mn,hn)):(mn=$[63],hn=$[64]),s(mn,hn);let gn,_n;J1($,65,L)?(gn=()=>()=>L?.close(),_n=[L],K2($,65,L,gn,_n)):(gn=$[66],_n=$[67]),s(gn,_n);let vn,yn;J2($,68,F,L)?(vn=()=>{L?.postMessage({type:"mode",on:F})},yn=[L,F],K3($,68,F,L,vn,yn)):(vn=$[70],yn=$[71]),s(vn,yn);let bn=H!==null||K!==null,xn,Sn;J2($,72,bn,L)?(xn=()=>{L?.postMessage({type:"composing",open:bn})},Sn=[L,bn],K3($,72,bn,L,xn,Sn)):(xn=$[74],Sn=$[75]),s(xn,Sn);let Cn,wn;J2($,76,Yt,L)?(Cn=()=>{L?.postMessage({type:"pins",pins:Yt})},wn=[L,Yt],K3($,76,Yt,L,Cn,wn)):(Cn=$[78],wn=$[79]),s(Cn,wn);let Tn,En;J2($,80,L,rn)?(Tn=()=>{L?.postMessage({type:"quotes",quotes:rn})},En=[L,rn],K3($,80,L,rn,Tn,En)):(Tn=$[82],En=$[83]),s(Tn,En);let Dn,On;J1($,84,L)?(Dn=()=>{L&&ut.current>0&&L.postMessage({type:"restoreScroll",y:ut.current})},On=[L],K2($,84,L,Dn,On)):(Dn=$[85],On=$[86]),s(Dn,On);let kn,An;J2($,87,H,L)?(kn=()=>{H===null&&L?.postMessage({type:"clearSelection"})},An=[L,H],K3($,87,H,L,kn,An)):(kn=$[89],An=$[90]),s(kn,An);let jn=F&&he&&!H&&!K,Mn=J2($,91,F,I)?K2($,91,F,I,()=>{I(!F)}):$[93],Q=J3($,94,O,j,P)?K3($,94,O,j,P,e=>{let t=O.current?.getBoundingClientRect();t&&(j.set(N(e.clientX-t.left+12,M,Math.max(M,t.width-224-M))),P.set(N(e.clientY-t.top+14,M,Math.max(M,t.height-80))))}):$[97],Nn=J0($,98)?K0($,98,e=>{q(t=>t===e?null:e),U(gt),Te(null)}):$[98],Pn=J0($,99)?K0($,99,()=>q(null)):$[99],Fn=J1($,100,V)?K1($,100,V,()=>{if(!V?.anchor)return;if(oe(V.anchor.exact,1e3)===""){Ot(null),R.current?.postMessage({type:"clearTextSel"});return}let e=Ke.current??{w:600,h:400};U({kind:"text",captureId:Rt.current+=1,boundsW:e.w,boundsH:e.h,anchor:V.anchor,fallbackX:N(V.rect.x+V.rect.w/2,0,e.w),fallbackY:V.rect.y}),Nt(""),It(V.anchor.exact.length>60),q(null),Ot(null),R.current?.postMessage({type:"clearTextSel"})}):$[101],In=H!==null,Ln,Rn;J4($,102,F,In,K,I)?(Ln=()=>{if(!F&&!In&&!K)return;let e=e=>{e.key!=="Escape"||e.defaultPrevented||e.isComposing||(e.preventDefault(),In||K?(U(null),q(null)):I(!1))};return window.addEventListener("keydown",e),()=>window.removeEventListener("keydown",e)},Rn=[F,In,K,I],K5($,102,F,In,K,I,Ln,Rn)):(Ln=$[106],Rn=$[107]),s(Ln,Rn);let zn=J0($,108)?K0($,108,e=>{e.key!=="Escape"||e.nativeEvent.isComposing||!Ve.current&&!kt.current&&!zt.current||(e.preventDefault(),e.stopPropagation(),kt.current||zt.current?(R.current?.postMessage({type:"clearSelection"}),U(null),q(null)):Tt.current?.(!1))}):$[108],Bn,Vn;J1($,109,k)?(Bn=()=>(k(zn),()=>k(null)),Vn=[k,zn],K2($,109,k,Bn,Vn)):(Bn=$[110],Vn=$[111]),s(Bn,Vn);let Hn,Un;J1($,112,F)?(Hn=()=>{U(_t),q(null),!F&&Ut.current&&(Ot(null),R.current?.postMessage({type:"clearTextSel"})),F||(ge(!1),Ce(null),Te(null))},Un=[F],K2($,112,F,Hn,Un)):(Hn=$[113],Un=$[114]),s(Hn,Un);let Wn,Gn;J2($,115,X,Z)?(Wn=()=>{Te(e=>e&&!X.some(t=>t.id===e)&&!Z.some(t=>t.id===e)?null:e)},Gn=[X,Z],K3($,115,X,Z,Wn,Gn)):(Wn=$[117],Gn=$[118]),s(Wn,Gn);let Kn,qn;J2($,119,X,z)?(Kn=()=>{Lt(e=>e&&X.some(t=>!e.knownIds.has(t.id)&&z?.has(t.id))?null:e)},qn=[X,z],K3($,119,X,z,Kn,qn)):(Kn=$[121],qn=$[122]),s(Kn,qn);let Jn,Yn;J1($,123,J)?(Jn=()=>{if(!J)return;let e=J.captureId,t=setTimeout(()=>{Lt(t=>t?.captureId===e?null:t)},2500);return()=>clearTimeout(t)},Yn=[J],K2($,123,J,Jn,Yn)):(Jn=$[124],Yn=$[125]),s(Jn,Yn);let Xn=J5($,126,_,H,W,g,X)?K5($,126,_,H,W,g,X,()=>{!H||!W.trim()||!_||(H.kind==="element"?(_.onCommentSubmit(ae(g,H.anchor),W.trim(),{displayMode:"rendered",forceNew:!0}),Lt({captureId:H.captureId,x:H.x,y:H.y,knownIds:new Set(X.map(vt))})):_.onCommentSubmit(le(g,{...H.anchor,prefix:"",suffix:""}),W.trim(),{displayMode:"rendered",forceNew:!0}),U(null),Nt(""))}):$[131],Zn,Qn;J3($,132,c,I,w)?(Zn=()=>{(w||!c)&&(I(!1),U(null),q(null),Lt(null),Ot(null))},Qn=[w,c,I],K4($,132,c,I,w,Zn,Qn)):(Zn=$[135],Qn=$[136]),s(Zn,Qn);let $n=J0($,137)?K0($,137,()=>{U(null),Nt(""),q(null),Lt(null),Ot(null),Dt(null)}):$[137],er=J1($,138,Y)?K1($,138,Y,[Y]):$[139];if(s($n,er),!(c&&_&&!w))return null;let tr;if(J5($,140,ln,F,_,H,Ft,W,O,G,Xn,Q,Fn,Mn,jn,we,A,T,Le,J,K,X,z,j,P,Z,B,Jt,V,D)){let e=K?ln.find(e=>e.id===K):void 0,n=J2($,169,K,z)?K2($,169,K,z,K?z?.get(K):void 0):$[171],i=J2($,172,K,B)?K2($,172,K,B,K?B?.get(K):void 0):$[174],s=i?.found?i.lineRects[0]:void 0,c=J4($,175,ln,H,we,K)?K4($,175,ln,H,we,K,we&&we!==K&&!H?ln.find(e=>e.id===we):void 0):$[179],l=J3($,180,ln,z,B)?K3($,180,ln,z,B,ln.filter(e=>{let t=z?.get(e.id)??B?.get(e.id);return t!==void 0&&!t.found})):$[183],p,g,v,y,b,x,S,C;if(J5($,184,F,H,Ft,W,O,G,l,Xn,Q,Fn,Mn,jn,c,A,T,Le,J,K,X,z,j,P,Z,B,V,D)){let e=B?.get(pe),n=H?.kind==="text"?e?.found&&e.lineRects[0]?{x:e.lineRects[0].x,y:e.lineRects[0].y+e.lineRects[0].h}:{x:H.fallbackX,y:H.fallbackY}:null,i=!F&&V!==null&&V.rect.w>0&&V.rect.h>0&&H===null&&K===null,s=H?.kind==="element"?{x:H.x,y:H.y}:n,_=J1($,218,H)?K1($,218,H,H?.kind==="element"?H.anchor.textHint?`\u201C${H.anchor.textHint}\u201D`:"":H?oe(H.anchor.exact,me):""):$[219],w=Le&&"hidden",ne=J1($,220,w)?K1($,220,w,t("absolute top-2 end-2 z-20",w)):$[221],ie=F?"border-accent-pro-200 bg-fill-pro text-oncolor-100":"border-border-300 bg-surface-3 text-primary hover:bg-surface-1",ae=J1($,222,ie)?K1($,222,ie,t("rounded-md border px-3 py-1.5 text-sm font-medium shadow-md",ie)):$[223],se=J1($,224,F)?K1($,224,F,F?r(re,{defaultMessage:"Done commenting",id:"cXIcxYfeiJ"}):r(re,{defaultMessage:"Comment",id:"YvbDJF7NMN"})):$[225],ce=J4($,226,F,Mn,ae,se)?K4($,226,F,Mn,ae,se,r("button",{type:"button","aria-pressed":F,onClick:Mn,className:ae,children:se})):$[230];J2($,231,ne,ce)?(v=r("div",{className:ne,children:ce}),K2($,231,ne,ce,v)):v=$[233];let le=J2($,234,jn,A)?K2($,234,jn,A,jn&&A&&r("div",{className:"pointer-events-none absolute",style:{left:A.x,top:A.y,width:A.w,height:A.h,border:`2px solid ${ve}`}})):$[236],ue;if(J3($,237,Q,T,K)||J3($,240,X.length,Z,B)){let e=J3($,244,Q,T,K)||J2($,247,X.length,B)?I2($,247,Z3($,244,(e,t)=>{let n=B?.get(e.id);if(!n?.found||n.lineRects.length===0)return null;let i=n.lineRects[0];return a("div",{className:"contents",children:[n.lineRects.map(yt),r("button",{type:"button",className:"pointer-events-auto absolute bg-transparent",style:{left:je(i.x-1,0,_e),top:je(i.y-23,0,_e)},onClick:()=>Nn(e.id),onMouseEnter:t=>{Q(t),Te(e.id)},onMouseMove:Q,onMouseLeave:()=>Te(t=>t===e.id?null:t),"aria-label":T.formatMessage({defaultMessage:"Comment {number} on selected text",id:"c2nYJ59GyU"},{number:X.length+t+1}),children:r(nt,{n:X.length+t+1,active:K===e.id})})]},e.id)},Q,T,K),X.length,B):$[249];ue=Z.map(e),K2($,237,Q,T,K),K3($,240,X.length,Z,B,ue)}else ue=$[243];let de;if(J5($,250,Q,T,J,K,X,z)){let e=J5($,257,Q,T,J,K,z)?K5($,257,Q,T,J,K,z,(e,t)=>{let n=z?.get(e.id);return!n||!n.found?null:r("button",{type:"button",className:"pointer-events-auto absolute bg-transparent",style:{left:je(n.x-1,0,_e),top:je(n.y-23,0,_e)},onClick:()=>Nn(e.id),onMouseEnter:t=>{Q(t),Te(e.id)},onMouseMove:Q,onMouseLeave:()=>Te(t=>t===e.id?null:t),"aria-label":T.formatMessage({defaultMessage:"Comment {number}",id:"BV8rl3I9dc"},{number:t+1}),children:r(nt,{n:t+1,active:K===e.id,swingIn:J!==null&&!J.knownIds.has(e.id)})},e.id)}):$[262];de=X.map(e),K5($,250,Q,T,J,K,X,z,de)}else de=$[256];let fe=J1($,263,l)?K1($,263,l,l.length>0&&r("div",{className:"pointer-events-auto absolute bottom-2 start-2 flex max-h-40 flex-col gap-1 overflow-y-auto",children:l.map(e=>r("button",{type:"button",className:"flex items-center gap-1.5 rounded-full border border-dashed bg-surface-3 px-2 py-0.5 text-xs text-secondary hover:bg-surface-1",onClick:()=>Nn(e.id),children:r(re,{defaultMessage:"Detached comment",id:"//Wx7/3+2u"})},e.id))})):$[264];y=a("div",{className:"pointer-events-none absolute inset-0 z-10 overflow-hidden",children:[le,ue,H?.kind==="text"&&e?.found&&e.lineRects.map(bt),de,fe]}),J1($,265,H)?(b=H?.kind==="element"&&H.anchor.descriptor&&r(tt,{descriptor:H.anchor.descriptor,x:H.labelX,y:H.labelY}),K1($,265,H,b)):b=$[266];let k=J4($,267,Fn,T,i,V)?K4($,267,Fn,T,i,V,i&&V&&r(ee.div,{initial:{opacity:0,y:8,scale:.9},animate:{opacity:1,y:0,scale:1},exit:{opacity:0,scale:.9,transition:{duration:.1}},transition:{duration:.18,ease:[.17,.84,.44,1]},className:"absolute z-30 flex size-0 items-end justify-center",style:Me(V.rect.x+V.rect.w/2,V.rect.y-6),children:r(d,{elevation:"popover",className:"flex shrink-0 p-1",children:r(f,{variant:"ghost",iconOnly:!0,icon:"ChatAdd","aria-label":T.formatMessage({defaultMessage:"Comment on selection",id:"FOrqFcMnhI"}),onClick:Fn})})})):$[271];J1($,272,k)?(x=r(te,{children:k}),K1($,272,k,x)):x=$[273];let he=J2($,274,H,J)?K2($,274,H,J,((H?.kind==="element"?H:null)??J)&&r(ee.div,{"aria-hidden":!0,className:"pointer-events-none absolute z-20 size-6 rounded-full rounded-bl-[2px]",initial:{rotate:H?.kind==="element"?0:-90,opacity:1},animate:{rotate:-90},exit:J?{opacity:0,transition:{duration:.05}}:{opacity:0,scale:.6},transition:{type:"spring",stiffness:260,damping:16},style:{left:je(((H?.kind==="element"?H.x:void 0)??J?.x??0)-1,0,_e),top:je(((H?.kind==="element"?H.y:void 0)??J?.y??0)-23,0,_e),transformOrigin:"1px 23px",backgroundColor:"#fff",boxShadow:`0 0 0 1px ${ye}, ${be}`}},H?.captureId??J?.captureId)):$[276];J1($,277,he)?(S=r(te,{children:he}),K1($,277,he,S)):S=$[278],J4($,279,c,j,P,D)?(C=c&&r(ee.div,{initial:{opacity:0,scale:.98},animate:{opacity:1,scale:1},transition:Se.transition,className:"pointer-events-none absolute left-0 top-0 z-30 w-56 rounded-xl bg-surface-popover p-3",style:{x:j,y:P,boxShadow:xe},children:r("div",{dir:D,className:"line-clamp-4 whitespace-pre-wrap break-words text-[13px] leading-snug text-primary",children:c.text})}),K4($,279,c,j,P,D,C)):C=$[283],p=te,g=H&&r(ee.div,{drag:!0,dragMomentum:!1,dragConstraints:O,dragElastic:.05,initial:{scale:.25,opacity:0},animate:{scale:1,opacity:1},exit:{scale:.9,opacity:0},transition:{type:"spring",stiffness:420,damping:28},className:t("absolute z-30 flex w-64 cursor-grab flex-col gap-xs p-sm active:cursor-grabbing",u("popover")),style:{...Ae({x:s?.x??0,y:s?.y??0,boundsW:H.boundsW,boundsH:H.boundsH,estH:H.kind==="element"&&(G||H.anchor.textHint.length>60||$e(H.anchor,H.suspect,G).show)?ke:H.kind==="element"||G?Oe:De}),transformOrigin:"0px 0px"},children:(()=>{let e=W.trim()!=="",n=xt,i=r("input",{ref:Pt,autoFocus:!0,type:"text",dir:Ft,className:"min-w-0 flex-1 cursor-text bg-transparent text-body text-primary outline-none placeholder:text-muted",value:W,"aria-label":T.formatMessage({defaultMessage:"Add a comment\u2026",id:"4oif0IPXeY"}),placeholder:T.formatMessage({defaultMessage:"Add a comment\u2026",id:"4oif0IPXeY"}),onPointerDownCapture:n,onChange:e=>Nt(e.target.value),onKeyDown:e=>{e.nativeEvent.isComposing||(e.key==="Enter"?(e.preventDefault(),Xn()):e.key==="Escape"&&kt.current&&(e.preventDefault(),e.stopPropagation(),U(null)))}}),s=r("div",{dir:D,className:"scroll-fade-y scroll-fade-size-3 max-h-[4lh] min-w-0 flex-1 overflow-y-auto whitespace-pre-wrap break-words rounded text-caption text-muted-as-secondary outline-none [scrollbar-width:thin] focus-visible:shadow-focus","data-quote-text":"","data-forced-text":G?"":void 0,tabIndex:0,onPointerDownCapture:n,children:_}),c=T.formatMessage({defaultMessage:"Add comment",id:"eiaPxp89eC"}),l=s;if(H.kind==="element"){let{suspect:e}=H,{tag:i,idPart:o,sectionPart:c,selectorLine:u,rawSelectorLine:d,show:p}=$e(H.anchor,e,G),g=i?et(i,T):null,v=i||E(H.anchor.descriptor.slice(0,12))||"?",y=i?`<${i}>`:"",b=u===""||u===i&&o===""&&c===""?y||v:[y,u].filter(Boolean).join(" \u00B7 "),x=T.formatMessage({defaultMessage:"Element context: {tag}",id:"5AVQs0KiSk"},{tag:g??v}),S=g!==null||i===""||qe.has(i);l=a("div",{className:"flex min-w-0 flex-1 flex-col",children:[a("div",{className:"flex min-w-0 flex-wrap items-start gap-x-xs",children:[r(m,{multiline:!0,content:r("span",{className:"break-all font-mono",children:b}),children:S?a(f,{variant:"ghost",size:"xxs",flush:!0,"aria-label":x,"data-element-chip":"",onPointerDownCapture:n,children:[r("span",{className:"-ms-0.5 inline-flex",children:r(h,{name:"Code",size:"auto-sm"})}),r("span",{"data-element-label":"",children:g??r("span",{className:"font-mono",children:v})})]}):r("button",{type:"button",className:"inline-flex min-w-0 max-w-full cursor-default select-none rounded text-start outline-none focus-visible:shadow-focus","aria-label":x,"data-element-chip":"",onPointerDownCapture:n,children:r("span",{className:"break-all font-mono text-caption font-medium text-secondary","data-element-label":"",children:v})})}),p&&r(m,{multiline:!0,content:r("span",{className:"break-all font-mono",children:d}),children:r("div",{className:t("scroll-fade-y scroll-fade-size-3 max-h-[4lh] min-w-32 flex-1 overflow-y-auto break-all rounded font-mono text-caption text-muted-as-secondary outline-none [scrollbar-width:thin] focus-visible:shadow-focus",g===null&&"basis-full"),"data-selector-line":"",tabIndex:0,onPointerDownCapture:n,children:u})})]}),G&&e.descriptor&&H.anchor.descriptor&&r("div",{className:"scroll-fade-y scroll-fade-size-3 max-h-[4lh] overflow-y-auto break-words rounded text-caption text-muted-as-secondary outline-none [scrollbar-width:thin] focus-visible:shadow-focus","data-raw-descriptor":"",tabIndex:0,onPointerDownCapture:n,children:H.anchor.descriptor}),_!==""&&s]})}return a(o,{children:[r("div",{className:"flex min-w-0 flex-col gap-xs rounded bg-alpha-1 px-sm py-xs",children:l}),a("div",{className:"flex items-center gap-xs ps-xs",children:[i,r("span",{className:"inline-flex shrink-0",onPointerDownCapture:n,children:r(f,{size:"xs",variant:e?"primary":"ghost",disabled:!e,"aria-label":c,onPointerDownCapture:n,onClick:Xn,children:r(re,{defaultMessage:"Add",id:"alWu/MbgXB"})})})]})]})})()},H.captureId),K5($,184,F,H,Ft,W,O,G,l,Xn,Q,Fn,Mn,jn,c,A,T,Le,J,K,X,z,j,P,Z,B,V,D,p,g,v,y,b,x,S,C)}else p=$[210],g=$[211],v=$[212],y=$[213],b=$[214],x=$[215],S=$[216],C=$[217];let w=J2($,284,p,g)?K2($,284,p,g,r(p,{children:g})):$[286];tr=a(o,{children:[v,y,b,x,S,C,w,e&&r(ct,{comment:e,replies:Jt.get(e.id)??ot,commentProps:_,x:n?.found?n.x+6:s?s.x+6:8,y:n?.found?n.y+6:s?s.y+s.h+6:8,onClose:Pn},e.id)]}),K5($,140,ln,F,_,H,Ft,W,O,G,Xn,Q,Fn,Mn,jn,we,A,T,Le,J,K,X,z,j,P,Z,B,Jt,V,D,tr)}else tr=$[168];return tr});c(function(e){let $=J9(12),t,n,i;J1($,0,e)?({className:n,children:t,...i}=e,K3($,0,e,t,n,i)):(t=$[1],n=$[2],i=$[3]);let{active:a,commentProps:o,uiHidden:s}=i,c=a&&!!o&&!(s!==void 0&&s),l=J1($,4,i)?K1($,4,i,e=>r(st,{...i,host:e})):$[5];return J5($,6,a,t,n,c,l)?K5($,6,a,t,n,c,l,r(de,{active:a,interactive:c,className:n,layer:l,children:t})):$[11]});var ct=c(function(e){let $=J9(66),{comment:t,replies:n,commentProps:o,x:s,y:c,onClose:l}=e,u=ie(),d=i()==="rtl"?"auto":void 0,f=t.target.anchor,p=J1($,0,t.id)||J1($,1,o.stagedIds)?I1($,1,Z1($,0,o.stagedIds?.has(t.id)??!1,t.id),o.stagedIds):$[2],g=J1($,3,s)?K1($,3,s,je(s,8,232)):$[4],_=J1($,5,c)?K1($,5,c,je(c,8,160)):$[6],v=J2($,7,g,_)?K2($,7,g,_,{left:g,top:_,boxShadow:xe}):$[9],y=J2($,10,t.text,d)?I2($,10,r("div",{dir:d,className:"whitespace-pre-wrap break-words text-[13px] leading-snug text-primary",children:t.text}),t.text,d):$[12],b;if(J2($,13,n,d)){let e=J1($,16,d)?K1($,16,d,e=>r("div",{className:"mt-2 ms-2 border-s-2 ps-2 text-[13px] leading-snug text-secondary",children:d===void 0?e.text:r("div",{dir:d,children:e.text})},e.id)):$[17];b=n.map(e),K2($,13,n,d,b)}else b=$[15];let x=J1($,18,f.descriptor)||J1($,19,f.kind)?I1($,19,Z1($,18,f.kind==="element"&&f.descriptor&&r(Qe,{descriptor:f.descriptor}),f.descriptor),f.kind):$[20],S=J1($,21,f.exact)||J2($,22,f.kind,d)?I2($,22,Z1($,21,f.kind==="text-quote"&&r("div",{dir:d,className:"truncate text-xs text-muted-as-secondary",children:`\u201C${oe(f.exact,80)}\u201D`}),f.exact),f.kind,d):$[24],te=J1($,25,t.createdAt)||J1($,26,u.locale)?I1($,26,Z1($,25,new Date(t.createdAt).toLocaleTimeString(u.locale,{hour:"numeric",minute:"2-digit"}),t.createdAt),u.locale):$[27],C=J1($,28,te)?K1($,28,te,r("span",{className:"shrink-0 text-[11px] text-muted-as-secondary",children:te})):$[29],w=J5($,30,t.id,o,u,p,l)?I5($,30,o.onSendToClaude&&!p&&r(m,{content:u.formatMessage({defaultMessage:"Send to QuixiDesk",id:"xGC0T/NxhG"}),children:r("button",{type:"button",className:"flex size-5 items-center justify-center rounded text-muted-as-secondary transition-colors hover:bg-surface-1 hover:text-primary",onClick:()=>{o.onSendToClaude?.(t.id),l()},"aria-label":u.formatMessage({defaultMessage:"Send to QuixiDesk",id:"xGC0T/NxhG"}),children:r(h,{name:"ChatAdd",size:"sm"})})}),t.id,o,u,p,l):$[35],T=J3($,36,t.id,o,l)?I3($,36,()=>{o.onCommentDelete(t.id),l()},t.id,o,l):$[39],ne=J1($,40,u)?K1($,40,u,u.formatMessage({defaultMessage:"Delete comment",id:"wOZRKWT1/R"})):$[41],re=J0($,42)?K0($,42,r(h,{name:"Trash",size:"sm"})):$[42],ae=J2($,43,T,ne)?K2($,43,T,ne,r("button",{type:"button",className:"flex size-5 items-center justify-center rounded text-muted-as-secondary transition-colors hover:bg-surface-1 hover:text-danger-100",onClick:T,"aria-label":ne,children:re})):$[45],E=J1($,46,u)?K1($,46,u,u.formatMessage({defaultMessage:"Close",id:"rbrahOGMC3"})):$[47],se=J0($,48)?K0($,48,r(h,{name:"X",size:"sm"})):$[48],ce=J2($,49,l,E)?K2($,49,l,E,r("button",{type:"button",className:"flex size-5 items-center justify-center rounded text-muted-as-secondary transition-colors hover:bg-surface-1 hover:text-primary",onClick:l,"aria-label":E,children:se})):$[51],le=J3($,52,w,ae,ce)?K3($,52,w,ae,ce,a("div",{className:"ms-auto flex shrink-0 items-center gap-0.5",children:[w,ae,ce]})):$[55],D=J4($,56,C,le,x,S)?K4($,56,C,le,x,S,a("div",{className:"mt-2 flex min-w-0 items-center gap-1.5",children:[x,S,C,le]})):$[60];return J4($,61,D,v,y,b)?K4($,61,D,v,y,b,a(ee.div,{...Se,className:"absolute z-30 w-56 rounded-xl bg-surface-popover p-3",style:v,children:[y,b,D]})):$[65]});function lt(e){return r("span",{className:"font-mono text-secondary",children:e})}function ut(e){return-e}function dt(e){let t=e.target.anchor;return{id:e.id,selector:t.selector,pinX:t.pinX,pinY:t.pinY}}function ft(e){return e.id}function pt(e){let t=e.target.anchor;return{id:e.id,exact:E(t.exact.slice(0,me)),prefix:E(t.prefix.slice(-32)),suffix:E(t.suffix.slice(0,32)),startOffset:Math.max(0,Math.trunc(Number(t.startOffset))||0)}}function mt(e){return e.id}function ht(e){return e?.kind==="element"?null:e}function gt(e){return e?.kind==="text"?e:null}function _t(e){return e?.kind==="text"?e:null}function vt(e){return e.id}function yt(e,t){return r("div",{className:"pointer-events-none absolute",style:{left:e.x,top:e.y,width:e.w,height:e.h,backgroundColor:Ce}},t)}function bt(e,t){return r("div",{className:"pointer-events-none absolute",style:{left:e.x,top:e.y,width:e.w,height:e.h,backgroundColor:we}},`draft-${t}`)}function xt(e){return e.stopPropagation()}var St="http://www.w3.org/2000/svg",Ct="--cwhr-cursor",wt="data-cwhr-local-mode",Tt="data-cwhr-local-composing";function z(e){let t=e.getAttribute("aria-label")??(e.namespaceURI===St?"":"innerText"in e&&typeof e.innerText=="string"?e.innerText:e.textContent??"");return S(t).replace(/\s+/g," ").trim()}function Et(e){return p(e)}function B(e,t){let n=t.closest('svg[role="img"]');if(n!==null&&e.contains(n))return n;let r=t;for(;;){let t=r.closest('[aria-hidden="true"]'),n=t?.parentElement;if(!t||!n||!e.contains(n)||t===e)return r;r=n}}function Dt(e){return e.replace(/\\/g,"\\\\").replace(/"/g,'\\"')}function V(e,t){let n=t=>{try{return e.querySelectorAll(t).length===1}catch{return!1}};for(let e of t.attributes)if(/^data-[a-zA-Z0-9_.:-]+$/.test(e.name)&&e.value!==""&&!/\s/.test(e.value)){let t=`[${e.name}="${Dt(e.value)}"]`;if(n(t))return t}let r=[],i=t;for(;i!==null&&i!==e&&r.length<10;){let e=i.tagName.toLowerCase(),t=i.parentElement;if(t!==null){let n=i.tagName,r=[...t.children].filter(e=>e.tagName===n);r.length>1&&(e+=`:nth-of-type(${r.indexOf(i)+1})`)}r.unshift(e),i=t}return i===e?r.join(" > "):":not(*)"}function Ot(e,t){try{return e.querySelector(t.startsWith("[")?t:`:scope > ${t}`)}catch{return null}}function H(e,t){let n=`<${e.tagName.toLowerCase()}>`,r=t.replace(/"/g,"'").slice(0,60);return r===""?n:`${n} "${r}"`}function U(e,t,{token:n,onCapture:r}){let i=e.ownerDocument,a=_(e),o=new a.MessageChannel,s=o.port1,c=!1,l=[],u=null,d=null,f=-1,p=-1,m=0,h=0,y=!1,b=e=>{y||s.postMessage(e)},x=()=>{let e=t.getBoundingClientRect();return{left:e.left,top:e.top}},S=()=>{m=0;let t=null;if(u!==null&&u!==e){let e=u.getBoundingClientRect();if(e.width>0||e.height>0){let n=x();t={x:e.left-n.left,y:e.top-n.top,w:e.width,h:e.height}}}b({type:"cursor",x:f,y:p,hover:t})},ee=()=>{m===0&&!y&&(m=a.requestAnimationFrame(S))},te=()=>{h=0;let t=x();if(d!==null){let n=d.getBoundingClientRect();b({type:"selRect",left:n.left-t.left,top:n.top-t.top,w:n.width,h:n.height,found:(n.width>0||n.height>0)&&e.contains(d)})}let n=l.map(n=>{let r=Ot(e,n.selector)?.getBoundingClientRect();return!r||r.width===0&&r.height===0?{id:n.id,x:0,y:0,found:!1}:{id:n.id,x:r.left-t.left+n.pinX*r.width,y:r.top-t.top+n.pinY*r.height,found:!0}});b({type:"rects",rects:n,quoteRects:[],scrollY:0})},C=()=>{h===0&&!y&&(h=a.requestAnimationFrame(te))},w=()=>{d=null},T=t=>{c=t,e.toggleAttribute(wt,t),t||(u=null,w())},ne=e=>{if(!c)return;let t=x();f=e.clientX-t.left,p=e.clientY-t.top,u=Et(e.target),ee()},re=()=>{c&&(f=-1,p=-1,u=null,ee())},ie=t=>{if(!c){let e=a.getSelection();(e===null||e.isCollapsed)&&b({type:"clickAway"});return}t.preventDefault(),t.stopPropagation();let n=Et(t.target);if(n===null||n===e)return;let i=B(e,n);if(i===e)return;let o=r?.(i)??"";d=i;let s=i.getBoundingClientRect(),l=x(),u=s.width>0?(t.clientX-s.left)/s.width:.5,f=s.height>0?(t.clientY-s.top)/s.height:.5,p=o.replace(/\s+/g," ").trim()||z(i);b({type:"capture",selector:V(e,i).slice(0,1e3),textHint:p.slice(0,300),descriptor:H(i,p),pinX:Math.min(1,Math.max(0,u)),pinY:Math.min(1,Math.max(0,f)),clientX:t.clientX-l.left,clientY:t.clientY-l.top,rectLeft:s.left-l.left,rectTop:s.top-l.top})},ae=t=>{if(t.key!=="Escape"||!c&&d===null)return;let n=Et(t.target);(t.target===i.body||n!==null&&e.contains(n))&&b({type:"dismiss"})};s.onmessage=t=>{let n=t.data;switch(n.type){case"mode":T(n.on);return;case"composing":e.toggleAttribute(Tt,n.open);return;case"pins":l=Array.isArray(n.pins)?n.pins:[],C();return;case"clearSelection":w();return;case"quotes":case"clearTextSel":case"restoreScroll":return}},e.style.setProperty(Ct,k),e.addEventListener("mousemove",ne,!0),e.addEventListener("mouseleave",re),e.addEventListener("click",ie,!0),i.addEventListener("keydown",ae,!0);let E=()=>{C(),c&&u!==null&&ee()};a.addEventListener("scroll",E,!0),a.addEventListener("resize",E);let oe=new(g(e))(E);oe.observe(e);let se=new(v(e))(C);return se.observe(e,{childList:!0,subtree:!0,attributes:!0,attributeFilter:["class","style","transform","d"]}),window.postMessage({type:O,token:n},window.location.origin,[o.port2]),{detach:()=>{y=!0,oe.disconnect(),se.disconnect(),T(!1),e.removeAttribute(Tt),e.style.removeProperty(Ct),e.removeEventListener("mousemove",ne,!0),e.removeEventListener("mouseleave",re),e.removeEventListener("click",ie,!0),i.removeEventListener("keydown",ae,!0),a.removeEventListener("scroll",E,!0),a.removeEventListener("resize",E),m!==0&&a.cancelAnimationFrame(m),h!==0&&a.cancelAnimationFrame(h),s.close()}}}export{st as HtmlPreviewReviewLayer,U as attachLocalHtmlReviewGuest,P as buildHtmlPreviewTextIndex,ge as injectHtmlPreviewCommentGuest};