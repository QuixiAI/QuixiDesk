import{A as e,M as t,N as n,T as r,u as i,y as a}from"./vendor-zod-fszujTq6.js";import{Co as o,Ha as s,To as c,ho as l,wo as u}from"./vendor-frame-CnSY1sxv.js";import{$f as J2,Jf as I3,Kf as I1,Nd as d,Qf as J1,Xf as I5,Zf as J0,ap as K0,cp as K3,dp as Z1,ep as J3,fp as Z2,hp as Z5,ip as J9,lp as K4,np as J5,op as K1,pp as Z3,sp as K2,tp as J4,up as K5}from"./shared-frame-6jtSkIit.js";import{En as f,Uh as p,Wh as m,Yh as h,rg as g,zr as _}from"./shared-25-DOWZRSlq.js";import{CS as v,LS as y,Vs as b,ht as x}from"./shared-0-DDOOdL6P.js";import{Ld as S,jv as C,xa as w}from"./shared-52-uHFNjl7C.js";import{$f as T,AE as E,DE as D,LC as O,ap as k,dl as A,gp as j,jE as ee,kE as M,mc as N}from"./shared-78-D5Fw2h0a.js";import{Ai as P}from"./shared-4-DUB1Oi0O.js";import{p as F}from"./vendor-intl-DYfox5Co.js";import{p as I,s as te}from"./vendor-query-DVJ0goAF.js";import{Eh as L,Uh as ne,is as R,os as re,ss as ie}from"./shared-3-BCEbd8Fv.js";import{Kd as z,Vc as B,Wd as V,Xf as H,ap as ae,au as oe,dd as U,ff as se,hc as ce,n_ as le,nm as ue,ru as de,tm as fe,yf as pe}from"./shared-156-BLTosN8U.js";import{Sl as me,Vl as he,Xl as ge}from"./shared-182-BI1KJccu.js";import{lc as _e}from"./shared-28-DNYlf4wu.js";import{c as W,n as ve,o as G,r as K,s as q,t as ye}from"./c363f4872-kRTAfCeZ.js";import{F as be,O as xe,P as Se,l as Ce}from"./c7cbb2ba7-DdI5t_bA.js";import{D as we,E as Te,T as Ee}from"./cd2c9366b-CrBf8kxu.js";var De=/^https?:\/\/[A-Za-z0-9.-]+(?::\d+)?\/(?:code\/)?artifact\/[A-Za-z0-9-]{1,64}$/;function Oe(e,t){return e===""||q(e)!==e||!De.test(t)?"":`Recreate this Doc in \u201C${e}\u201D using its connector\u2019s tools. When you need the Doc\u2019s content, read it with the Claude Docs read tool: ${t}`}var ke={rebuild:{variant:"doc_rebuild_in_partner",compose:Oe},pageRowsFirst:!0,besideConversationOnly:!0,readsWithDocsTools:!0},Ae="json",je="md";function Me(e,t,n){return{rules:e,routes:{importToolName:e.importToolName,mayRebuild:!0,list:t,format:n}}}var Ne=Me(ke,"docs",je);function Pe(e,t){let $=J9(3),n=e.typeSlug??e.namedTypeSlug,r=t&&n!==void 0?L(n,!0,void 0):void 0,i=r==="other",a=re(),o=ie(),s;bb0:{if(r==="pages"){s=Ne;break bb0}if(!i){s=void 0;break bb0}if(a===null){s=o?"withheld":void 0;break bb0}let e;if(J2($,0,a,n)){let t=a.sendToTypeRules(n);e=t===void 0?void 0:Me(t,"type",Ae),K2($,0,a,n,e)}else e=$[2];s=e}return s}function Fe(e){let $=J9(2),{state:t}=e;return J1($,0,t)?K1($,0,t,t==="withheld"?s(R,{}):null):$[1]}var Ie="artifact_send_to_partner",Le={"eb9240f2-e1c1-43c1-828f-0fda40c22e4c":"canva"};function Re(e,t=Date.now()){if(e===void 0||Number.isNaN(e.getTime()))return 10;let n=Math.round((e.getTime()-t)/6e4);return n>=1&&n<=10?n:10}function ze(e){try{let t=new URL(e),n=[...t.searchParams.keys()];return t.href!==e||t.hash!==""||n.length!==1||n[0]!=="t"?e:(t.search="",t.href)}catch{return e}}function Be(e){let{source:t,...n}=e,r=e.destination.handoffTokenParam!==void 0&&e.token!==void 0,i=t===void 0?"":Ve(n,{url:r?ze(t.url):t.url,...r&&t.token!==void 0&&{token:t.token}});return i===""?Ve(n,void 0):i}function Ve({destination:e,url:t,expiresAt:n,token:r,importTool:i,statusTool:a},o){let s=e.handoffTokenParam,c=s!==void 0&&r!==void 0;if(i!==void 0)return ye({destination:q(e.name),url:c?ze(t):t,minutes:Re(n),format:"other",importTool:i,...a===void 0?{}:{statusTool:a},...c?{token:{param:s,value:r}}:{}});let l=G(e.format);return ye({destination:q(e.name),url:c?ze(t):t,minutes:Re(n),format:l,hint:l==="mp4"?void 0:e.hint??Le[e.id],...c?{token:{param:s,value:r}}:{},...o===void 0?{}:{source:o}})}function He(){return Be}var Ue=200,We=/[\p{Cc}\u2028\u2029]/gu;function J(e){return[...e.replace(We," ").trim()].slice(0,Ue).join("").trim()}function Ge(e){return{sendTo:J(e.sendTo),destinations:J(e.destinations),connectMore:J(e.connectMore),more:J(e.more),connect:J(e.connect),newTab:J(e.newTab)}}function Ke(e,t){return t?w(e):e}function qe({offered:e,destinations:t,busy:n,desktop:r,copy:i,unavailable:a,pageFirst:o=!1,appsOff:s=!1}){if(!i||a===void 0&&(!t?.length||!e))return null;let c=[];for(let e of t??[]){let t=Ce(e.label??e.name);if(t==="")continue;let n={id:e.id,name:t,...e.iconUrl===void 0?{}:{icon:e.iconUrl}};e.status===void 0?e.connected?c.push({...n,state:"connected"}):a?.listUnconnected!=="listed"&&e.connectUrl!==void 0?c.push({...n,state:"connect",href:Ke(e.connectUrl,r)}):a!==void 0&&c.push({...n,state:"listed"}):c.push({...n,state:"listed",note:J(i.statusNotes[e.status])})}let l={...o?{pageFirst:!0}:{},...s?{appsOff:!0}:{}};return a===void 0?{mode:"live",destinations:c,copy:Ge(i),...n?{busy:!0}:{},...l}:{mode:"unavailable",destinations:c,copy:Ge(i),unavailable:Je(a),...l}}function Je(e){return{reason:Ze.test(e.reason)?e.reason:"unavailable",...e.note===void 0?{}:{note:J(e.note)},...e.action===void 0?{}:{action:{kind:e.action.kind,label:J(e.action.label)}}}}function Ye(e,t,n){let r=e?.contentWindow;if(!r)return!1;let i={__frame_chrome:!0,cmd:"send_to",uuid:t,offer:n};return r.postMessage(i,location.origin),!0}var Xe=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i,Ze=/^[A-Za-z0-9_-]{1,64}$/,Qe=new Set(["send","connect","browse","retry"]);function $e(e,t){if(typeof e!="object"||!e)return null;let n=e;return n.__frame_chrome_state!==!0||n.state!=="send_to"||typeof n.uuid!="string"||!Xe.test(n.uuid)||n.uuid.toLowerCase()!==t.toLowerCase()||typeof n.action!="string"||!Qe.has(n.action)?null:n.action==="send"||n.action==="connect"?typeof n.destination=="string"&&Ze.test(n.destination)?{action:n.action,destination:n.destination}:null:n.destination===void 0?{action:n.action}:null}function et(e,t){return e===t||JSON.stringify(e)===JSON.stringify(t)}function tt(e,t){let $=J9(23),{uuid:n,offer:r,resetKey:i,onRequest:a}=t,s=d()??window,c=o(a),u,p;J1($,0,a)?(u=()=>{c.current=a},p=[a],K2($,0,a,u,p)):(u=$[1],p=$[2]),l(u,p);let m=f(r,et),h=J1($,3,i)?K1($,3,i,{key:i,sent:!1}):$[4],g=o(h),_,v;J5($,5,s,e,i,m,n)?(_=()=>{if(!n)return;g.current.key!==i&&(g.current={key:i,sent:!1});let t=()=>{m===null&&!g.current.sent||Ye(e.current,n,m)&&(g.current.sent=m!==null||g.current.sent)};t();let r=n=>{let r=e.current?.contentWindow,i=n.data;r&&n.source===r&&n.origin===location.origin&&typeof i=="object"&&i&&i.__frame_chrome_state===!0&&(i.state==="comment_mode"||i.state==="version")&&t()};return s.addEventListener("message",r),()=>s.removeEventListener("message",r)},v=[e,n,m,i,s],K5($,5,s,e,i,m,n,_,v)):(_=$[10],v=$[11]),l(_,v);let y=o(n),b,x;J1($,12,n)?(b=()=>{y.current=n},x=[n],K2($,12,n,b,x)):(b=$[13],x=$[14]),l(b,x);let S,C;J1($,15,e)?(S=()=>()=>{y.current&&g.current.sent&&Ye(e.current,y.current,null)},C=[e],K2($,15,e,S,C)):(S=$[16],C=$[17]),l(S,C);let w,T;J3($,18,s,e,n)?(w=()=>{if(!n)return;let t=t=>{let r=e.current?.contentWindow;if(!r||t.source!==r||t.origin!==location.origin)return;let i=$e(t.data,n);i!==null&&c.current(i)};return s.addEventListener("message",t),()=>s.removeEventListener("message",t)},T=[e,n,s],K4($,18,s,e,n,w,T)):(w=$[21],T=$[22]),l(w,T)}var nt=`
document.addEventListener('DOMContentLoaded', async function() {
  const loading = document.getElementById('__bundler_loading');
  function setStatus(msg) { if (loading) loading.textContent = msg; }

  const FONT_MIME = /^(font[/]|application[/](x-)?font-|application[/]vnd\\.ms-fontobject)/i;
  const MIME_TOKEN = /^[\\w.+-]+[/][\\w.+-]+$/;
  function toBase64(bytes) {
    let bin = '';
    for (let i = 0; i < bytes.length; i += 0x8000) {
      bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    }
    return btoa(bin);
  }

  // Error sink persists across replaceWith since it's on window, not the DOM.
  window.addEventListener('error', function(e) {
    // Failed resource loads (CSP-blocked links, scripts, images) fire plain
    // events at the element \u2014 warn only; real JS errors carry message/error.
    if (!e.message && !e.error && e.target && e.target !== window) {
      console.warn('[bundle] resource failed to load:',
        e.target.tagName, String(e.target.src || e.target.href || ''));
      return;
    }
    var p = document.body || document.documentElement;
    var d = document.getElementById('__bundler_err') || p.appendChild(document.createElement('div'));
    d.id = '__bundler_err';
    d.style.cssText = 'position:fixed;bottom:12px;left:12px;right:12px;font:12px/1.4 ui-monospace,monospace;background:#2a1215;color:#ff8a80;padding:10px 14px;border-radius:8px;border:1px solid #5c2b2e;z-index:99999;white-space:pre-wrap;max-height:40vh;overflow:auto';
    d.textContent = (d.textContent ? d.textContent + String.fromCharCode(10) : '') +
      '[bundle] ' + (e.message || e.type) +
      (e.filename ? ' (' + e.filename.slice(0, 60) + ':' + e.lineno + ')' : '');
  }, true);

  try {
    const manifestEl = document.querySelector('script[type="__bundler/manifest"]');
    const templateEl = document.querySelector('script[type="__bundler/template"]');
    if (!manifestEl || !templateEl) {
      setStatus('Error: missing bundle data');
      console.error('[bundler] Missing script tags \u2014 manifestEl:', !!manifestEl, 'templateEl:', !!templateEl);
      return;
    }

    const manifest = JSON.parse(manifestEl.textContent);
    let template = JSON.parse(templateEl.textContent);

    // Nested page bundles (iframe targets). Each ships ONCE, in the ROOT
    // document's manifest; frame srcs carry an about:blank#<uuid> marker
    // instead of a substitutable uuid. Every document mints the blobs for
    // its OWN frames \u2014 from local text at the root, or text obtained from
    // its DIRECT parent over the relay protocol below \u2014 so each frame
    // navigation stays same-origin with its initiator and a deduped page
    // loads at any depth on any host, including a file:// download's
    // opaque origins (where a blob minted by one document cannot be
    // navigated to by another).
    const pageOrderEl = document.querySelector('script[type="__bundler/page_order"]');
    const pageOrder = pageOrderEl ? JSON.parse(pageOrderEl.textContent) : [];
    const pageSet = new Set(pageOrder);
    const pageTexts = {};

    // \u2500\u2500 Nested-page text protocol (parent-chain relay) \u2500\u2500
    // Page bundles ship once, in the ROOT document's manifest. Every
    // document mints the blobs for its OWN frames, obtaining page text
    // through a strict parent-chain relay: a document only ever SENDS to
    // its direct parent (which, for every document the publisher's own
    // runtime minted, is another publisher document) and only ACCEPTS
    // text from that direct parent; requests from below are honored only
    // for the document's own child frames, and the ROOT never sends
    // upward at all. That last rule is load-bearing: the artifact host's
    // frame-ancestors allows claude.ai (and friends) to frame an
    // artifact, so window.top can be a FOREIGN document \u2014 a top-addressed
    // protocol would hand page uuids to that ancestor and accept forged
    // "page text" from it, minting attacker HTML as an artifact-origin
    // blob document. The chain never touches the ancestor: requests stop
    // at the root, replies are origin-addressed per hop (never '*' when
    // an origin exists), and both directions are origin-gated (on https
    // a foreign sender's origin can never equal this document's; in
    // opaque contexts \u2014 file:// \u2014 everything serializes as 'null', where
    // an embedded EXTERNAL frame still fails the gate by carrying its
    // own real origin, and a sandboxed child of such a frame is excluded
    // by the own-child-frames check plus uuid unguessability). Shape is
    // deny-by-default: one request form, one reply form, no error
    // replies.
    // window.origin is the SERIALIZED SECURITY origin ('null' in opaque
    // contexts such as a sandboxed preview iframe) \u2014 location.origin is
    // computed from the URL and stays 'https://\u2026' even when the document
    // is sandboxed, which would misgate every opaque chain.
    const OWN_TARGET = /^https?:[/][/]/.test(window.origin || '')
      ? window.origin
      : '*';
    function trustedOrigin(e) {
      if (e.origin === window.origin) return true; // https chains; opaque==opaque
      // file:// trees: Chrome serializes the file document's origin as
      // 'file://' while the blob: documents it mints serialize as 'null',
      // so the two legitimate shapes can never string-match \u2014 accept the
      // pair in both directions. https origins never serialize as either,
      // and sender identity (own child frame / direct parent) is checked
      // separately.
      return (
        (e.origin === 'null' &&
          (window.origin === 'file://' ||
            window.location.protocol === 'file:')) ||
        (/^file:/.test(e.origin) && window.origin === 'null')
      );
    }
    // Non-empty only in the root document (nested bundles ship an empty
    // island), so it doubles as the root marker: the root answers from
    // local text and treats a miss as authoritative \u2014 never relaying to
    // its (possibly foreign) parent.
    const isPageRoot = pageOrder.length > 0;
    const blobUrls = {};
    const resourceBlobs = {};
    const UUID_SHAPE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
    const pendingFrames = {}; // uuid -> [{el, frag}] awaiting text for own frames
    const pendingRelays = {}; // uuid -> [childWindow] awaiting text to pass down
    function mountPage(el, frag, text) {
      const pageBlob = new Blob([text], { type: 'text/html' });
      const u = URL.createObjectURL(pageBlob);
      resourceBlobs[u] = pageBlob;
      el.src = u + frag;
    }
    function deliverPage(uuid, text) {
      const frames = pendingFrames[uuid];
      delete pendingFrames[uuid];
      if (frames) for (const f of frames) mountPage(f.el, f.frag, text);
      const relays = pendingRelays[uuid];
      delete pendingRelays[uuid];
      if (relays) {
        for (const cw of relays) {
          try { cw.postMessage({ __bundler_page: uuid, __bundler_text: text }, OWN_TARGET); } catch (err) { /* gone */ }
        }
      }
    }
    function ownFrameWindows() {
      const out = [];
      for (const f of Array.from(document.querySelectorAll('iframe'))) {
        try { if (f.contentWindow) out.push(f.contentWindow); } catch (err) { /* detached */ }
      }
      return out;
    }
    window.addEventListener('message', function (e) {
      const d = e.data;
      if (!d || typeof d !== 'object' || !e.source) return;
      // Origin gate first; the structural checks below (own-child-frames
      // for requests, direct-parent + solicited-uuid for replies, the
      // root's never-relay rule) are what hold in fully opaque chains
      // where origin strings can't discriminate \u2014 a sandboxed root's
      // FOREIGN wrapper still fails all three even though both serialize
      // as real-vs-'null' rather than matching.
      if (!trustedOrigin(e)) return;
      if (typeof d.__bundler_need === 'string' && UUID_SHAPE.test(d.__bundler_need)) {
        // Requests are honored only for this document's own child frames.
        if (ownFrameWindows().indexOf(e.source) < 0) return;
        const uuid = d.__bundler_need;
        const text = pageTexts[uuid];
        if (typeof text === 'string') {
          try { e.source.postMessage({ __bundler_page: uuid, __bundler_text: text }, OWN_TARGET); } catch (err) { /* gone */ }
          return;
        }
        if (isPageRoot || window.parent === window) return; // authoritative miss
        const waiting = pendingRelays[uuid];
        if (waiting) {
          if (waiting.indexOf(e.source) < 0) waiting.push(e.source);
          return;
        }
        // Never-answered uuids would otherwise accumulate forever under a
        // request spray; past the cap (far above any real page count) new
        // relays are dropped rather than remembered.
        if (Object.keys(pendingRelays).length >= 128) return;
        pendingRelays[uuid] = [e.source];
        window.parent.postMessage({ __bundler_need: uuid }, OWN_TARGET);
      } else if (
        typeof d.__bundler_page === 'string' &&
        UUID_SHAPE.test(d.__bundler_page) &&
        typeof d.__bundler_text === 'string'
      ) {
        // The shape test is load-bearing beyond tidiness: the solicited
        // check below uses the in operator, which matches prototype keys
        // ('__proto__', 'constructor') \u2014 an unshaped key would reach
        // deliverPage and throw, painting the error banner.
        // Text is accepted only from the direct parent \u2014 the document
        // that minted this one.
        if (e.source !== window.parent || window.parent === window) return;
        if (!(d.__bundler_page in pendingFrames) && !(d.__bundler_page in pendingRelays)) return;
        deliverPage(d.__bundler_page, d.__bundler_text);
      }
    });

    const uuids = Object.keys(manifest);
    setStatus('Unpacking ' + uuids.length + ' assets...');

    await Promise.all(uuids.map(async (uuid) => {
      const entry = manifest[uuid];
      try {
        const binaryStr = atob(entry.data);
        const bytes = new Uint8Array(binaryStr.length);
        for (let i = 0; i < binaryStr.length; i++) bytes[i] = binaryStr.charCodeAt(i);

        let finalBytes = bytes;
        if (entry.compressed) {
          if (typeof DecompressionStream !== 'undefined') {
            const ds = new DecompressionStream('gzip');
            const writer = ds.writable.getWriter();
            const reader = ds.readable.getReader();
            writer.write(bytes);
            writer.close();
            const chunks = [];
            let totalLen = 0;
            while (true) {
              const { done, value } = await reader.read();
              if (done) break;
              chunks.push(value);
              totalLen += value.length;
            }
            finalBytes = new Uint8Array(totalLen);
            let offset = 0;
            for (const chunk of chunks) { finalBytes.set(chunk, offset); offset += chunk.length; }
          } else {
            console.warn('DecompressionStream not available, asset ' + uuid + ' may not render');
          }
        }

        if (pageSet.has(uuid)) {
          pageTexts[uuid] = new TextDecoder().decode(finalBytes);
          return;
        }
        if (FONT_MIME.test(entry.mime) && MIME_TOKEN.test(entry.mime)) {
          // Strict artifact hosts allow font-src data: but not blob:.
          blobUrls[uuid] = 'data:' + entry.mime + ';base64,' +
            (entry.compressed ? toBase64(finalBytes) : entry.data);
        } else {
          const blob = new Blob([finalBytes], { type: entry.mime });
          blobUrls[uuid] = URL.createObjectURL(blob);
          resourceBlobs[blobUrls[uuid]] = blob;
        }
      } catch (err) {
        console.error('Failed to decode asset ' + uuid + ':', err);
        const blob = new Blob([], { type: entry.mime });
        blobUrls[uuid] = URL.createObjectURL(blob);
        resourceBlobs[blobUrls[uuid]] = blob;
      }
    }));

    const extResEl = document.querySelector('script[type="__bundler/ext_resources"]');
    const extResources = extResEl ? JSON.parse(extResEl.textContent) : [];
    const resourceMap = {};
    for (const entry of extResources) {
      if (blobUrls[entry.uuid]) resourceMap[entry.id] = blobUrls[entry.uuid];
    }

    // Artifact-host CSP (connect-src 'self') refuses fetch() of blob: URLs \u2014
    // consumers read these Blobs directly. Survives the swap like the error sink.
    window.__resourceBlobs = resourceBlobs;

    setStatus('Rendering...');
    // Page uuids have no blob here \u2014 they ride inside about:blank#<uuid>
    // frame markers the pass below resolves after the swap.
    for (const uuid of uuids) {
      if (pageSet.has(uuid)) continue;
      template = template.split(uuid).join(blobUrls[uuid]);
    }

    // Strip integrity + crossorigin \u2014 blob URLs from a file:// document inherit
    // a null origin, so crossorigin forces a CORS fetch that SRI then rejects.
    // The manifest bytes are ours; SRI protects against CDN compromise, not this.
    // appifacts: only on tags now pointing at OUR blob: URLs. A reference the
    // bundler left live-fetched (an absolute x-import, a runtime dependency it
    // could not embed, an authored external script) keeps the pin it was
    // written with: that fetch is exactly what SRI protects. (A crossorigin
    // on a parent whose child carries the blob \u2014 <video crossorigin><source>
    // \u2014 is left alone too; the attribute is then the author's to drop.)
    template = template.replace(/<[a-z][^>]*>/gi, (tag) => /\\s(?:src|href|srcset|poster|data|xlink:href)="blob:/i.test(tag) ? tag.replace(/\\s+integrity="[^"]*"/gi, '').replace(/\\s+crossorigin="[^"]*"/gi, '') : tag);

    const resourceScript = '<script>window.__resources = ' +
      JSON.stringify(resourceMap).replace(/<\\//g, '<\\\\/') +
      ';</' + 'script>';
    // Inject after <head> so the DOCTYPE stays first; prepending the script
    // would push the parser into quirks mode. DOMParser always emits a <head>
    // (synthesizing one if the source HTML omitted it) but may carry
    // attributes through, so match the full opening tag. slice() rather than
    // replace() keeps us clear of $-pattern substitution in resourceScript.
    const headOpen = template.match(/<head[^>]*>/i);
    if (headOpen) {
      const i = headOpen.index + headOpen[0].length;
      template = template.slice(0, i) + resourceScript + template.slice(i);
    }

    // Parse the template and swap the root element. Scripts inserted via
    // DOMParser/replaceWith are inert per spec \u2014 re-create each with
    // createElement so they execute, awaiting onload for src scripts to
    // preserve ordering (React before ReactDOM before Babel before text/babel).
    const doc = new DOMParser().parseFromString(template, 'text/html');
    document.documentElement.replaceWith(doc.documentElement);

    // Resolve page-frame markers: mint this document's own blob for each
    // framed page \u2014 from local text when this is the root, otherwise by
    // asking the direct parent over the relay protocol above. An
    // unanswered request leaves the frame on about:blank, the same
    // degraded state as a refused embed.
    {
      const MARKER = /^about:blank#([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})(#.*)?$/;
      const needed = [];
      for (const el of Array.from(document.querySelectorAll('iframe'))) {
        const m = MARKER.exec(el.getAttribute('src') || '');
        if (!m) continue;
        const text = pageTexts[m[1]];
        if (typeof text === 'string') {
          mountPage(el, m[2] || '', text);
        } else if (window.parent !== window && !isPageRoot) {
          const had = pendingFrames[m[1]];
          (pendingFrames[m[1]] = had || []).push({ el: el, frag: m[2] || '' });
          if (!had) needed.push(m[1]);
        }
      }
      for (const uuid of needed) {
        window.parent.postMessage({ __bundler_need: uuid }, OWN_TARGET);
      }
    }

    const dead = Array.from(document.scripts);
    for (const old of dead) {
      const s = document.createElement('script');
      for (const a of old.attributes) s.setAttribute(a.name, a.value);
      s.textContent = old.textContent;
      // text/babel scripts with a src: read and inline. transformScriptTags
      // does XHR against the src, but blob:null/ from a file:// origin is
      // silently dropped. Inlining makes it a plain inline babel script,
      // which transformScriptTags handles unconditionally.
      if ((s.type === 'text/babel' || s.type === 'text/jsx') && s.src) {
        const pre = resourceBlobs[s.src.split('#')[0]];
        s.textContent = pre ? await pre.text() : await (await fetch(s.src)).text();
        s.removeAttribute('src');
      }
      const p = s.src ? new Promise(function(r) { s.onload = s.onerror = r; }) : null;
      old.replaceWith(s);
      if (p) await p;
    }
    // Babel standalone auto-transforms type=text/babel on DOMContentLoaded,
    // which fired before we swapped the document. Trigger manually if present.
    if (window.Babel && typeof window.Babel.transformScriptTags === 'function') {
      window.Babel.transformScriptTags();
    }
  } catch (err) {
    setStatus('Error unpacking: ' + err.message);
    console.error('Bundle unpack error:', err);
  }
});
`,rt="#faf9f5",it=`<svg viewBox="0 0 1920 1080" xmlns="http://www.w3.org/2000/svg"><rect width="1920" height="1080" fill="${rt}"/></svg>`,at="[]",ot=/<script\s+type\s*=\s*["']?__bundler\/manifest["'\s>]/i,st=/<script\s+type\s*=\s*["']?__bundler\/template["'\s>]/i,ct=/data:([\w.+-]+\/[\w.+-]+);base64,([A-Za-z0-9+/]+={0,2})/g,lt=/^(font[/]|application[/](x-)?font-|application[/]vnd\.ms-fontobject)/i,ut=1024;function dt(e){let t=e.length%4==0?e.replace(/={1,2}$/,""):e;return t.length%4!=1&&!t.includes("=")}function ft(e,t){let n={};return{template:e.replace(ct,(e,r,i)=>{if(!t.test(r)||i.length<ut||!dt(i))return e;let a=crypto.randomUUID();return n[a]={mime:r,compressed:!1,data:i},a}),manifest:n}}function pt(e){return e.replace(/</g,"\\u003c")}function mt(e){return e.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;").replace(/'/g,"&#39;")}function ht(e){return ot.test(e)&&st.test(e)}function gt(e,{title:t,keepInlineFonts:n=!1}){let{template:r,manifest:i}=n?{template:e,manifest:{}}:ft(e,lt);return`<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>${mt(t)}</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { background: ${rt}; display: flex; align-items: center; justify-content: center; min-height: 100vh; font-family: -apple-system, BlinkMacSystemFont, sans-serif; }
    #__bundler_loading { position: fixed; bottom: 20px; right: 20px; font: 13px/1.4 -apple-system, BlinkMacSystemFont, sans-serif; color: #666; background: #fff; padding: 8px 14px; border-radius: 8px; box-shadow: 0 1px 4px rgba(0,0,0,0.12); z-index: 10000; }
    #__bundler_thumbnail { position: fixed; inset: 0; width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; background: ${rt}; z-index: 9999; }
    #__bundler_thumbnail svg { width: 100%; height: 100%; object-fit: contain; }
    #__bundler_placeholder { color: #999; font-size: 14px; }
  </style>
  <noscript>
    <style>#__bundler_loading { display: none; }</style>
    <div style="position:fixed;bottom:12px;left:12px;font:13px/1.4 -apple-system,BlinkMacSystemFont,sans-serif;color:#999;background:rgba(255,255,255,0.9);padding:6px 12px;border-radius:6px;box-shadow:0 1px 4px rgba(0,0,0,0.08);z-index:10000;">
      This page requires JavaScript to display.
    </div>
  </noscript>
</head>
<body>
  <div id="__bundler_thumbnail">${it}</div>
  <div id="__bundler_loading">Unpacking...</div>

  <script>
    ${nt}
  <\/script>

  <script type="__bundler/manifest">
${pt(JSON.stringify(i))}
  <\/script>

  <script type="__bundler/ext_resources">
${at}
  <\/script>

  <script type="__bundler/page_order">
${at}
  <\/script>

  <script type="__bundler/template">
${pt(JSON.stringify(r))}
  <\/script>
</body>
</html>`}function _t(e,t){let n=new TextDecoder().decode(e);if(ht(n))return e;let r=new TextEncoder().encode(gt(n,t));return r.byteOffset===0&&r.byteLength===r.buffer.byteLength?r.buffer:r.slice().buffer}var vt=80,yt=/[\p{Cc}\p{Cf}\u2028\u2029]/gu;function bt(e){return typeof e=="string"?[...e.replace(yt," ").replace(/\s+/g," ").trim()].slice(0,vt).join("").trim():""}var xt=64,St=3e5,Ct=12e3;function wt(e){if(!(typeof e!="string"||e.length>2048))try{let t=new URL(e);return t.protocol==="https:"?t.href:void 0}catch{return}}function Tt(e){let t=wt(e);if(t===void 0)return;let n=new URL(t).hostname,r=typeof window>"u"?null:window.location.hostname;if(v(n,r)===null&&n!==r)return t}var Et=64;function Dt(e,t,n){let r=N(e.iconUrl??t,e.url,e.verifiedTier);if(r.iconType==="external")return r.iconSrc;if(!(!n||r.iconSrc===void 0||Ot(r.iconSrc)))return D(r.iconSrc,Et/2)}function Ot(e){try{let t=new URL(e).hostname;return v(t,null)!==null||t==="anthropic.com"||t.endsWith(".anthropic.com")}catch{return!0}}var kt=/^\/(?![/\\])/;function At(e){if(!(typeof e!="string"||e.length>2048||!kt.test(e)))try{let t=new URL(e,location.origin);if(t.origin!==location.origin)return;let n=t.pathname+t.search+t.hash;return kt.test(n)||n==="/"?n:void 0}catch{return}}function jt(e){let t=At(e);return t!==void 0&&Mt.test(t)?t:void 0}var Mt=/^\/directory\/[A-Za-z0-9][A-Za-z0-9._~-]*(?:[?#]|$)/,Nt=/^[A-Za-z0-9][A-Za-z0-9._~-]*$/,Pt="omelette_send_to_partner_ids",Ft="__gb__2111295379",It="__gb__462357012",Lt="22854937-9510-4b57-9230-62c820102d8f",Rt=new Set([Lt]),zt="cai_glistening_moon",Bt="__gb__1196304285",Vt="custom-",Ht=5,Ut=/[\p{L}\p{N}]+/gu;function Wt(e){Ut.lastIndex=0;let t=0;for(;Ut.exec(e)!==null;)t++;return t}var Y=a(()=>i(n())),Gt=a(()=>t([e(),r({id:e()}).passthrough()])),Kt=new Map([["html",new Set(["import-claude-design-from-url","import-design-from-url"])],["mp4",new Set(["import-claude-motion-from-url"])]]),qt=new Map([["import-design-from-url","canva"]]),Jt=new Map([["html","text/html"],["json","application/json"],["mp4","video/mp4"]]),Yt=["html"],Xt=new Map,Zt=new Map,Qt=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,$t=[],en=a(()=>i(e()));function tn(e){if(e===void 0)return;let t=new Set;for(let n of Array.isArray(e)?e:[])typeof n=="string"&&t.add(n.trim().toLowerCase());return[...Jt.keys()].filter(e=>t.has(e))}var nn=["licensed","none"],rn="json";function an(e){if(typeof e!="object"||!e||!("fonts"in e))return;let t=new Set;for(let n of Array.isArray(e.fonts)?e.fonts:[])typeof n=="string"&&t.add(n.trim().toLowerCase());let n=nn.filter(e=>t.has(e));if(n.length===0)return;let r="edition"in e?e.edition:1;return typeof r=="number"&&Number.isSafeInteger(r)&&r>=1?{format:rn,fonts:n,edition:r}:void 0}var on=["coming_soon"];function sn(e){let t=bt(e);return t!==""&&q(t)===t&&Ce(t)===t?t:void 0}function cn(e){let t=Gt.safeParse(e);if(!t.success)return;if(typeof t.data=="string")return{id:t.data.trim().toLowerCase(),show:!1,accepts:void 0,tokenParam:void 0,source:void 0,label:void 0,status:void 0};let n=t.data.status,r=typeof n=="string"?n.trim().toLowerCase():n,i=t.data.handoff_token_param;return{id:t.data.id.trim().toLowerCase(),show:t.data.show===!0,accepts:tn(t.data.accepts),tokenParam:ve(i)?i:void 0,source:an(t.data.code_file),label:sn(t.data.label),status:r==null||r===""?void 0:on.find(e=>e===r)??"unknown"}}function ln(e,t=[],n=Xt){let r=Y.safeParse(e);if(!r.success)return[];let i=new Set(Rt),a=new Set;for(let e of t){let t=cn(e);t!==void 0&&(t.show&&t.status!=="unknown"?a:i).add(t.id)}for(let e of a)i.delete(e);let o=[],s=new Set;for(let e of r.data){let t=cn(e);if(t===void 0||s.has(t.id))continue;let{id:r}=t;if(s.add(r),t.status!=="unknown"&&Qt.test(r)&&!i.has(r)&&n.get(r)?.length!==0&&o.push(r),o.length>=xt)break}return o}var un=new Map,dn=new Map,fn=new Map,pn=new Map;function X(e,t){let n=new Map,r=Y.safeParse(e);if(!r.success)return n;let i=new Set;for(let e of r.data){let r=cn(e);if(r===void 0||i.has(r.id))continue;i.add(r.id);let a=t(r);a!==void 0&&n.set(r.id,a)}return n}function mn(e){return X(e,e=>e.tokenParam)}function hn(e){return X(e,e=>e.source)}function gn(e){return X(e,e=>e.label)}function _n(e){return X(e,({status:e})=>e==="unknown"?void 0:e)}function vn(e){return X(e,e=>e.accepts)}function yn(e,t){let n=pe(e.uuid,t)?.uuid??B(t,[e]).get(e.uuid)?.[0],r=n===void 0?void 0:t.remoteServers[n];if(n===void 0||r===void 0||k(r))return;let i=t.remoteConnectionStates[n];return{connection:{isConnected:i?.isConnected??!1,connectionState:i?.connectionState,authStatus:i?.authStatus,connectionError:i?.connectionError},tools:[...t.remoteTools[n]??[],...t.remoteAppOnlyTools[n]??[]],toolsListed:n in t.remoteTools,iconUrl:t.remoteIconUrls[n]}}function bn(e,t,n,r,i,a=un,o=Xt,s,c=dn,l=fn,u=pn){let d=e=>s===void 0?Kt.get(e):new Set(s.importToolName===void 0?[]:[s.importToolName]),f=[];for(let p of e){let e=t.get(p);if(e===void 0)continue;let m=s===void 0?(o.get(p)??Yt).filter(e=>e!==rn):o.get(p)??[s.format],h=bt(e.name);if(!h||q(h)===""||W(h)===""&&Ce(h)!==h)continue;let g=a.get(p),_=s===void 0?c.get(p):void 0,v=l.get(p),y=u.get(p),b={...v===void 0?{}:{label:v},...y===void 0?{}:{status:y},...g===void 0?{}:{handoffTokenParam:g},..._===void 0?{}:{source:_}},x=yn(e,n),S=y===void 0&&x!==void 0&&r&&A(x.connection),C=Tt(Dt(e,x?.iconUrl,i)),w=m.filter(e=>x?.tools.some(t=>d(e)?.has(t.name)===!0&&!j(t))!==!0);if(w.length===0)continue;let T=s?.importToolName===void 0?void 0:x?.tools.find(e=>e.name===s.importToolName);if(x!==void 0&&S){let e;if(s!==void 0&&T===void 0){if(!s.mayRebuild||!x.toolsListed)continue;e="rebuild"}f.push({id:p,name:h,iconUrl:C,accepts:w,connected:!0,connectUrl:void 0,...b,...e===void 0?{}:{route:e}});continue}f.push({id:p,name:h,iconUrl:C,accepts:w,connected:!1,connectUrl:y===void 0?jt(`/directory/${e.slug!==void 0&&Nt.test(e.slug)?e.slug:p}`):void 0,...b})}return f}function xn(e){let t=e.inputSchema,n=typeof t=="object"&&t&&"properties"in t?t.properties:void 0;if(!(typeof n!="object"||!n))return Object.hasOwn(n,"download_token")?"download_token":void 0}function Sn(e,t,n,r){if(!n)return[];let i=r===void 0?Kt:r.importToolName===void 0?new Map:new Map([[r.format,new Set([r.importToolName])]]),a=[...e.values()],o=new Set([...B(t,a).values()].flat()),s=e=>W(e).normalize("NFKD").replace(/\p{M}/gu,"").toLowerCase(),c=new Set(a.map(e=>s(bt(e.name)))),l=[],u=Object.entries(t.remoteServers).sort(([,e],[,t])=>(e?.name??"").localeCompare(t?.name??""));for(let[e,n]of u){if(l.length>=xt)break;let a=t.remoteConnectionStates[e],u=bt(n.name),d=W(u),f=s(u);if(n.mcp_directory_server_uuid!==null&&a?.isDirectoryServer!==!1||o.has(e)||k(n)||typeof n.mcp_directory_server_uuid=="string"||a===void 0||a.isDirectoryServer===!0||typeof a.directoryEntryUuid=="string"||T({uuid:e,isDirectoryServer:a.isDirectoryServer})||!A(a)||!u||Ce(u)!==u||d===""||Wt(d)>Ht||c.has(f))continue;let p=t.remoteTools[e]??[],m=[];for(let[e,t]of i)for(let n of t){let t=p.find(e=>e.name===n&&j(e));if(t!==void 0){m.push([e,t]);break}}if(m.length===0)continue;let h=m.every(([,e])=>xn(e)!==void 0)?"download_token":void 0,g=r===void 0?m.map(([,e])=>qt.get(e.name)).find(e=>e!==void 0):void 0;c.add(f),l.push({id:Vt+e.toLowerCase(),name:u,iconUrl:void 0,accepts:m.map(([e])=>e),connected:!0,connectUrl:void 0,...h===void 0?{}:{handoffTokenParam:h},...g===void 0?{}:{hint:g}})}return l}function Cn(e,t,n,r,i,a){return t?[...e,...Sn(n,r,i,a)].slice(0,xt):[...e]}function wn(e,t){let n=(t??[]).filter(e=>e.disabled!==!0).map(e=>e.format);if(n.length===0)return[];let r=[];for(let t of e){if(!t.connected&&t.connectUrl===void 0&&t.status===void 0)continue;let e=n.find(e=>t.accepts.includes(e));if(e===void 0)continue;let{source:i,...a}=t,o=i!==void 0&&e==="mp4"&&n.includes(i.format);r.push({...a,format:e,...o?{source:i}:{}})}return r}function Tn(e,t,n){return wn(e,(t??[]).map(e=>({format:e.format}))).filter(e=>!n.some(t=>t.id===e.id))}function En(e,t){let n=e.filter(e=>e.status===void 0);return(t??[]).some(e=>e.format==="webm"&&e.disabled!==!0)&&wn(n,t).length===0&&wn(n,[{format:"mp4"}]).length>0}function Dn(e,t){let $=J9(24),n=J0($,0)?K0($,0,[]):$[0],r=m(Pt,n,Y,e&&t===void 0),i=J0($,1)?K0($,1,[]):$[1],a=m(Ft,i,Y,e&&t==="type"),o=J0($,2)?K0($,2,[]):$[2],s=m(It,o,Y,e&&t==="docs"),c=t==="docs"?s:t==="type"?a:r,l=J0($,3)?K0($,3,[]):$[3],u=m(zt,l,Y,e),d=g(Bt,e),f,p;J2($,4,u,c)?(f=vn(c),p=ln(c,u,f),K3($,4,u,c,f,p)):(f=$[6],p=$[7]);let h=J1($,8,c)?K1($,8,c,mn(c)):$[9],_=J1($,10,c)?K1($,10,c,hn(c)):$[11],v=J1($,12,c)?K1($,12,c,gn(c)):$[13],y=J1($,14,c)?K1($,14,c,_n(c)):$[15];return J5($,16,f,p,h,_,v,y,d)?K5($,16,f,p,h,_,v,y,d,{ids:p,tokenParams:h,sources:_,labels:v,statuses:y,accepts:f,testConnectors:d}):$[23]}function On(e){let $=J9(76),{enabled:t,orgUuid:n,exports:r,routes:i}=e,a=(J1($,0,r)?K1($,0,r,r??[]):$[1]).length>0,o=O("mcp_artifacts"),s=t&&!!n&&a,d=s&&o.isAvailable,{ids:f,tokenParams:m,sources:h,labels:g,statuses:_,accepts:v,testConnectors:y}=Dn(d,i?.list),b=p("conditional_mcp_directory_servers","visibility",$t,en,d),x=J1($,2,f)?K1($,2,f,f.join(",")):$[3],S=f.length>0&&b.length>0,C=J1($,4,b)?K1($,4,b,b.join(",")):$[5],w=J2($,6,x,C)?K2($,6,x,C,["artifact-handoff-directory",x,C]):$[8],T=J2($,9,x,b)?K2($,9,x,b,async()=>{let e=await ce({ids:x,visibility:b.join(","),verified_tier:"anthropic,partner,community"}),t=new Map;for(let n of e.servers)n.type==="remote"&&Qt.test(n.uuid)&&t.set(n.uuid.toLowerCase(),n);return t}):$[11],D=d&&S,k=J0($,12)?K0($,12,{noToast:!0}):$[12],A=J3($,13,w,T,D)?K3($,13,w,T,D,{queryKey:w,queryFn:T,enabled:D,staleTime:St,refetchOnWindowFocus:!0,retry:1,meta:k}):$[16],j=te(A),N=ue(),P=c(ee,M,Nn),F,I;J2($,17,d,P)?(I=()=>{d&&!P&&E()},F=[d,P],K3($,17,d,P,F,I)):(F=$[19],I=$[20]),l(I,F);let L=oe(),ne=J1($,21,d)?K1($,21,d,{enabled:d}):$[22],R=de(ne),re=fe(Mn),ie=ge(),z=fe(jn)||d&&R&&!L&&!re&&!ie,B=fe(An),V=d?j.data:void 0;Fn(J4($,23,V,d,n,R)?K4($,23,V,d,n,R,{live:d,orgUuid:n,listings:V,statesKnown:R}):$[27]);let H=d&&V===void 0&&!j.isError&&(j.isFetching||j.isPaused),[ae,U]=u(!1),[se,le]=u(H);H!==se&&(le(H),H||U(!1));let[pe,me]=u(0),he=J1($,28,H)?K1($,28,H,()=>{if(!H)return;let e=setTimeout(()=>U(!0),Ct);return()=>clearTimeout(e)}):$[29],_e=J2($,30,pe,H)?K2($,30,pe,H,[H,pe]):$[32];l(he,_e);let W=d&&V===void 0&&(j.isError||H&&ae),ve=j.refetch,G=J4($,33,W,ve,B,z)?K4($,33,W,ve,B,z,()=>{z&&B(),W&&(U(!1),me(kn),ve())}):$[37],K=S||y,q;bb0:{let e=!S||V!==void 0,t=J5($,38,o.status,K,W,R,z,e,s)?I5($,38,In({wanted:s,connectorsFeature:o.status,configured:K,storeUnknown:z,directoryFailed:W,listingsLoaded:e,statesKnown:R}),o.status,K,W,R,z,e,s):$[45];if(t!=="ready"||!K){let e=J1($,46,t)?K1($,46,t,t==="ready"?[]:void 0):$[47];q=J3($,48,G,t,e)?K3($,48,G,t,e,{status:t,destinations:e,needsMp4:!1,retry:G}):$[51];break bb0}let n=V??Zt,a,c,l,u;if(J5($,52,v,r,g,N,f,P,n,i,h,R,t,_,y,m)){let e=Cn(bn(f,n,N,R,P,m,v,i,h,g,_),y,n,N,R,i),o=wn(e,r);a=t,c=o,l=Tn(e,r,o),u=En(e,r),K5($,52,v,r,g,N,f,P,n,i,h,R,t,_,y,m,a,c,l,u)}else a=$[66],c=$[67],l=$[68],u=$[69];q=J5($,70,G,a,c,l,u)?K5($,70,G,a,c,l,u,{status:a,destinations:c,waiting:l,needsMp4:u,retry:G}):$[75]}return q}function kn(e){return e+1}function An(e){return e.retryRemoteBootstrap}function jn(e){return e.remoteStreamErrored||e.remoteBootstrapShed}function Mn(e){return e.remoteHasBootstrapped}function Nn(){return!1}function Pn(e,t,n){e.add(n),V(t,{serverUuid:n,onTerminal:()=>e.delete(n)})}function Fn(e){let $=J9(29),{live:t,orgUuid:n,listings:r,statesKnown:i}=e,a=I(),s=z(),c=_().activeOrganization?.uuid,u=J3($,0,c,t,n)?K3($,0,c,t,n,t&&n!==void 0&&n===c?.toLowerCase()?c:void 0):$[3],d=h(P),f=u!==void 0&&d,p=J1($,4,f)?K1($,4,f,{enabled:f,refetchOnMount:"always",refetchOnWindowFocus:"always"}):$[5],m=he(u,p),g=J1($,6,r)?K1($,6,r,r===void 0?void 0:[...r.values()]):$[7],v=J0($,8)?K0($,8,new Set):$[8],y=o(v),x=J0($,9)?K0($,9,new Set):$[9],S=o(x),C=J3($,10,g,u,a)||J3($,13,m.data,i,s)?I3($,13,Z3($,10,()=>{if(u===void 0||!i||g===void 0||m.data===void 0)return;let e=se.getState(),t={};for(let e of m.data)t[e.uuid]=e;let n=B({remoteServers:t,remoteDirectories:e.remoteDirectories,remoteConnectionStates:e.remoteConnectionStates},g),r=!1;for(let i of n.values())for(let n of i){let i=t[n],a=`${n}@${i?.state_changed_at??""}|${i?.last_connected_at??""}`,o=e.remoteConnectionStates[n],c=e.remoteServers[n]!==void 0;i?.connection_state!=="connected"||y.current.has(n)||S.current.has(a)||k(e.remoteServers[n])||s&&o?.isConnecting===!0||c&&n in e.remoteTools&&A({isConnected:o?.isConnected??!1,connectionState:o?.connectionState,authStatus:o?.authStatus,connectionError:o?.connectionError})||(S.current.add(a),s?Pn(y.current,u,n):c?o!==void 0&&!o.isConnected&&(o.isConnecting||U(n)?H(n):ae(n)):r=!0)}r&&a.invalidateQueries({queryKey:[b,u]})},g,u,a),m.data,i,s):$[16],w=J3($,17,g,u,a)||J1($,20,m.data)||J3($,21,m.dataUpdatedAt,i,s)?I3($,21,Z1($,20,Z3($,17,[u,i,g,m.data,m.dataUpdatedAt,s,a],g,u,a),m.data),m.dataUpdatedAt,i,s):$[24];l(C,w);let T,E;J2($,25,u,a)?(E=()=>{if(u===void 0||typeof BroadcastChannel>"u")return;let e=new BroadcastChannel(le);return e.onmessage=e=>{let t=e.data;typeof t!="object"||!t||t.type!=="mcp-auth-complete"||t.step!=="success"||typeof t.serverId!="string"||!Qt.test(t.serverId)||a.invalidateQueries({queryKey:me(u)})},()=>e.close()},T=[u,a],K3($,25,u,a,T,E)):(T=$[27],E=$[28]),l(E,T)}function In(e){return e.wanted?e.connectorsFeature==="available"?e.configured?e.storeUnknown?"store_error":e.directoryFailed?"directory_error":!e.listingsLoaded||!e.statesKnown?"loading":"ready":"ready":e.connectorsFeature==="blocked_by_org_admin"?"admin_off":"off":"off"}var Z={phase:"idle"},Ln={invalid_request:"unsupported",too_large:"too_large",rate_limited:"rate_limited",not_found:"unsupported",not_permitted:"not_permitted",org_disabled:"org_disabled",unavailable:"unavailable",internal:"network"},Rn=16777216,zn=2e5;function Bn(e,t){return`/api/frame/handoff/${encodeURIComponent(e)}?org=${encodeURIComponent(t)}`}var Vn=new Set(["rate_limited","unavailable"]),Hn=3,Un=6e4,Wn=1e4;function Gn(e,t){let n=Q(t)&&Q(t.error)?t.error:void 0,r=n&&typeof n.code=="string"?n.code:void 0;if(r!==void 0&&Ln[r]!==void 0)return Ln[r];switch(e){case 413:return"too_large";case 429:return"rate_limited";case 403:return r===void 0&&!Q(t)?"refused":"not_permitted";case 404:return n?.type==="not_found_error"&&r===void 0?"network":"unsupported";case 400:case 415:return"unsupported";case 503:return"unavailable";default:return e>=500||e===0?"network":"unsupported"}}function Q(e){return typeof e=="object"&&!!e&&!Array.isArray(e)}var Kn=/^\.frame\.(?:staging\.)?claudeusercontent\.com$/,qn="/_handoff/";function Jn(e,t){let n=wt(e);if(n===void 0||!t)return;let r=new URL(n);if(r.username!==""||r.password!==""||r.port!=="")return;let i=t.toLowerCase();return r.hostname.startsWith(`${i}.`)&&Kn.test(r.hostname.slice(i.length))&&r.pathname.startsWith(qn)?r.href:void 0}function Yn(e,t){let n=new URL(t).searchParams.get("t");return n===null?K(e)?e:void 0:K(n)?n:void 0}function Xn(e,t){if(!Q(e))return null;let n=Jn(e.url,t);if(n===void 0)return null;let r;if(typeof e.expires_at=="string"){let t=new Date(e.expires_at);r=Number.isNaN(t.getTime())?void 0:t}let i=Yn(e.token,n);return{url:n,expiresAt:r,...i===void 0?{}:{token:i}}}async function Zn({orgUuid:e,artifactUuid:t,handoffToken:n,filename:r,destinationId:i,bytes:a,contentType:o,surface:s}){let c=await x(Bn(t,e),{method:"POST",headers:{...ne(s),"Content-Type":o,"X-Frame-Handoff-Token":n,"X-Frame-Handoff-Filename":encodeURIComponent(r),"X-Frame-Handoff-Destination":i},body:a}),l=await c.json().catch(()=>void 0);if(c.ok)return{ok:!0,body:l};let u=Q(l)&&Q(l.error)?l.error.code:void 0,d=(c.status===429||c.status===503)&&typeof u=="string"&&Vn.has(u),f={ok:!1,error:Gn(c.status,l),stale:c.status===403&&!Q(l)};if(!d)return f;let p=Number(c.headers.get("Retry-After"));return{...f,retryAfterMs:Number.isFinite(p)?Math.max(1,p)*1e3:1e3}}function Qn(e,t){return new Promise(n=>{if(t.aborted){n();return}let r=()=>{clearTimeout(i),t.removeEventListener("abort",r),n()},i=setTimeout(r,e);t.addEventListener("abort",r,{once:!0})})}async function $n(e,t,n,r){let i=Date.now(),a=e,o=!1,s=0;for(;;){let e=await Zn(a);if(!e.ok&&e.stale&&!o){o=!0;let i=await r(a.handoffToken,t);if(t.aborted||!n())return"abandoned";if(i===void 0)return e;a={...a,handoffToken:i};continue}if(s+=1,e.ok||e.retryAfterMs===void 0||s>=Hn)return e;let c=e.retryAfterMs+Math.random()*1e3;if(t.aborted||!n())return"abandoned";if(Date.now()-i+c>Un)return e;if(await Qn(c,t),t.aborted||!n())return"abandoned"}}function er(e){return e.replace(/\.html?$/i,"")||e}var tr=new Set([Lt]);function nr(e,t){return e.format==="html"?_t(e.bytes,{title:er(e.filename),keepInlineFonts:tr.has(t)}):e.bytes}var rr=300,ir=2e4,ar=5e3,or=1e3,sr=1e4;function cr(e,t){let n;try{n=JSON.parse(new TextDecoder().decode(e))}catch{return!1}if(!Q(n))return!1;let{format:r,edition:i,fontsRule:a}=n;return r==="claude-motion-source"&&typeof i=="number"&&Number.isSafeInteger(i)&&i>=t.edition&&t.fonts.some(e=>e===a)}async function lr({rule:e,destination:t,requestExport:n,signal:r,assistant:i}){if(r.aborted)return{ok:!1,reason:"unbound"};let a=new AbortController,o=()=>a.abort(),s=!1;r.addEventListener("abort",o,{once:!0});let c=i?setTimeout(()=>{s=!0,o()},ir):void 0,l;try{l=await n({format:e.format,destination:t,signal:a.signal,...i&&{assistant:!0}})}catch{l={ok:!1,reason:"failed"}}finally{clearTimeout(c),r.removeEventListener("abort",o)}return l.ok?l.bytes.byteLength>16777216?{ok:!1,reason:"too_large"}:cr(l.bytes,e)?{ok:!0,filename:l.filename,bytes:l.bytes}:{ok:!1,reason:"rule_unmet"}:{ok:!1,reason:s?"timeout":l.reason}}async function ur(e,t){try{let n=await Zn(e);return n.ok?Xn(n.body,t)??"link_failed":n.error}catch{return"network"}}function dr(e){return e<262144?"lt_256k":e<1048576?"lt_1m":e<4194304?"lt_4m":e<=16777216?"lt_16m":"gt_16m"}async function fr({destination:e,orgUuid:t,artifactUuid:n,handoffToken:r,renewToken:i,requestExport:a,composePrompt:o,importTool:s,statusTool:c,surface:l,signal:u,assistant:d,live:f,onUploading:p,onSource:m}){let h=Jt.get(e.format);if(h===void 0)return{kind:"error",error:"unsupported"};we(h)&&Ee(t);let g=e.source===void 0?void 0:Jt.get(e.source.format),_,v=e.source!==void 0&&g!==void 0;if(e.source!==void 0&&g!==void 0){let t=await lr({rule:e.source,destination:e.label??e.name,requestExport:a,signal:u,assistant:d});if(t.ok)_=t;else if(m(t.reason),t.reason==="unbound")return{kind:"abandoned"};if(await Qn(rr,u),!f())return{kind:"abandoned"}}let y,b=Date.now(),x=!1,S=!1;for(;;){try{y=await a({format:e.format,destination:e.label??e.name,signal:u,...d&&{assistant:!0}})}catch{y={ok:!1,reason:"failed"}}if(!(v&&!y.ok&&(y.reason==="unsupported"||y.reason==="failed"&&!x)&&Date.now()-b<sr))break;if(!y.ok&&y.reason==="failed"&&(x=!0),S=!0,await Qn(or,u),!f())return{kind:"abandoned"}}if(!y.ok)return y.reason==="declined"||y.reason==="expired"||y.reason==="page_cancelled"?{kind:y.reason}:y.reason==="unbound"?{kind:"abandoned"}:{kind:"error",error:y.reason};if(S&&_!==void 0&&(m("replaced"),_=void 0),y.bytes.byteLength>16777216)return{kind:"error",error:"too_large",size:dr(y.bytes.byteLength)};let C;try{C=nr(y,e.id)}catch{return{kind:"error",error:"failed",size:dr(y.bytes.byteLength)}}let w=dr(C.byteLength);if(C.byteLength>16777216)return{kind:"error",error:"too_large",size:w};if(!f())return{kind:"abandoned",size:w};if(p(),we(h)){let e=new Blob([C],{type:h}),n=await Te("frame_send_to",t,e,{maxSignedBytes:Rn,stillWanted:f});if(n!==e){let e=C;C=await n.arrayBuffer().catch(()=>e)}if(!f())return{kind:"abandoned",size:w}}let T;try{let a=await $n({orgUuid:t,artifactUuid:n,handoffToken:r,filename:y.filename,destinationId:e.id,bytes:C,contentType:h,surface:l},u,f,i);if(a==="abandoned")return{kind:"abandoned",size:w};if(!a.ok)return{kind:"error",error:a.error,size:w};T=Xn(a.body,n)}catch{return{kind:"error",error:"network",size:w}}if(T===null)return{kind:"error",error:"link_failed",size:w};if(!f())return{kind:"abandoned",size:w};let E=null;if(_!==void 0&&g!==void 0){let i=new AbortController,a=()=>i.abort();u.addEventListener("abort",a,{once:!0});let o=await Promise.race([ur({orgUuid:t,artifactUuid:n,handoffToken:r,filename:_.filename,destinationId:e.id,bytes:_.bytes,contentType:g,surface:l},n),Qn(ar+(d?0:_.bytes.byteLength/1e3),i.signal).then(()=>"timeout")]);if(i.abort(),u.removeEventListener("abort",a),!f())return{kind:"abandoned",size:w};typeof o=="string"?m(o):E=o}let D={destination:e,url:T.url,expiresAt:T.expiresAt,...T.token===void 0?{}:{token:T.token},...s===void 0?{}:{importTool:s},...c===void 0?{}:{statusTool:c}},O=o(E===null?D:{...D,source:{url:E.url,...E.token===void 0?{}:{token:E.token}}});return O===""?{kind:"error",error:"unsupported",size:w}:(E!==null&&m(O===o(D)?"unsupported":"attached"),{kind:"link",prompt:O,size:w})}function pr(e,t,n){let r=t.compose(q(e.name),`${window.location.origin}${C(n)}`);return r===""?{kind:"error",error:"unsupported"}:{kind:"link",prompt:r}}function mr(e){let $=J9(46),{orgUuid:t,artifactUuid:n,handoffToken:r,refreshBoot:i,conversationUuid:a,laneKey:s,deliver:c,requestExport:d,composePrompt:f,typeRules:p,mayRebuild:m}=e,h=s===void 0?a:s,g=m!==void 0&&m,_=S(),v=_e(),[b,x]=u(Z),C=o(0),w=o(null),T=o(null),E=J0($,0)?K0($,0,()=>{C.current+=1,T.current?.abort(),x(Z)}):$[0],D=J0($,1)?K0($,1,()=>E):$[1],O=J3($,2,n,h,t)?K3($,2,n,h,t,[t,n,h,E]):$[5];l(D,O);let k=E,A=o(r),j=J0($,6)?K0($,6,new Set):$[6],ee=o(j),M,N;J1($,7,r)?(M=()=>{A.current=r;for(let e of ee.current)e()},N=[r],K2($,7,r,M,N)):(M=$[8],N=$[9]),l(M,N);let P=J1($,10,i)?K1($,10,i,(e,t)=>A.current===e?!i||t.aborted?Promise.resolve(void 0):(i(),new Promise(n=>{let r=ee.current,i=()=>{clearTimeout(o),r.delete(a),t.removeEventListener("abort",i),n(A.current===e?void 0:A.current)},a=()=>{A.current!==e&&i()},o=setTimeout(i,Wn);r.add(a),t.addEventListener("abort",i,{once:!0})})):Promise.resolve(A.current)):$[11],F=y(P),I=J1($,12,_)||J1($,13,b.phase)?I1($,13,Z1($,12,(e,t)=>{if(b.phase==="confirming"||b.phase==="uploading"||w.current===C.current)return null;let n=C.current+=1;w.current=n;let r=new AbortController;T.current=r;let i=Date.now(),a={destination_id:e.id,format:e.format,...t==="tool"?{via:t}:{},...e.route===void 0?{}:{route:e.route}};return x({phase:"confirming",destination:e}),_.track({event_key:"claudeai.jolly_candle.requested",...a}),{live:()=>C.current===n,signal:r.signal,settle:(e,t,o)=>{w.current===n&&(w.current=null),T.current===r&&(T.current=null),_.track({event_key:"claudeai.jolly_candle.settled",...a,outcome:e,duration_ms:Date.now()-i,...t===void 0?{}:{size_bucket:t},...o===void 0?{}:{source:o}})}}},_),b.phase):$[14],te=J5($,15,n,I,f,c,r,h,g,t,F,d,v)||J1($,26,p?.importToolName)||J1($,27,p?.rebuild)||J1($,28,p?.statusToolName)?I1($,28,Z1($,27,Z1($,26,Z5($,15,e=>{if(!t||!n||!r||!h||!d||!e.connected)return;let i=e.route==="rebuild",a=i&&g?p?.rebuild:void 0;if(i&&a===void 0)return;let o=I(e,"row");if(o===null)return;let{live:s}=o,l,u=(e,t)=>o.settle(e,t,l);(async()=>{let i=a===void 0?await fr({destination:e,orgUuid:t,artifactUuid:n,handoffToken:r,renewToken:F,requestExport:d,composePrompt:f,importTool:p?.importToolName,statusTool:p?.statusToolName,surface:v,signal:o.signal,assistant:!1,live:s,onUploading:()=>x({phase:"uploading",destination:e}),onSource:e=>{l=e}}):pr(e,a,n);if(i.kind==="error"){u(i.error,i.size),s()&&x({phase:"error",destination:e,error:i.error});return}if(i.kind!=="link"){u(i.kind,i.kind==="abandoned"?i.size:void 0),s()&&x(i.kind==="page_cancelled"?{phase:"cancelled"}:Z);return}let m;try{m=a===void 0?await c(i.prompt):await c(i.prompt,a.variant)}catch{u("network",i.size),s()&&x({phase:"error",destination:e,error:"network"});return}u(m===!1?"send_rejected":"handed",i.size),s()&&x(Z)})()},n,I,f,c,r,h,g,t,F,d,v),p?.importToolName),p?.rebuild),p?.statusToolName):$[29],L=J5($,30,n,I,f,r,h,t,F,d,v)||J1($,39,p?.importToolName)||J1($,40,p?.statusToolName)?I1($,40,Z1($,39,Z5($,30,async(e,i)=>{if(!t||!n||!r||!h||!d||!e.connected||e.route==="rebuild")return{kind:"unavailable"};if(i?.aborted)return{kind:"cancelled"};let a=I(e,"tool");if(a===null)return{kind:"in_progress"};let{live:o,settle:s}=a,c,l=()=>{o()&&E()};i?.addEventListener("abort",l,{once:!0});let u=!0,m=!1,g=setTimeout(()=>{u&&o()&&(m=!0,E())},zn),_=await fr({destination:e,orgUuid:t,artifactUuid:n,handoffToken:r,renewToken:F,requestExport:d,composePrompt:f,importTool:p?.importToolName,statusTool:p?.statusToolName,surface:v,signal:a.signal,assistant:!0,live:o,onUploading:()=>{u=!1,x({phase:"uploading",destination:e})},onSource:e=>{c=e}});switch(clearTimeout(g),i?.removeEventListener("abort",l),s(_.kind==="error"?_.error:_.kind==="link"?"linked":_.kind==="abandoned"&&m?"timed_out":_.kind,"size"in _?_.size:void 0,c),o()&&x(Z),_.kind){case"link":return{kind:"link",prompt:_.prompt};case"error":return{kind:"error",error:_.error};case"declined":return{kind:"declined"};case"expired":return{kind:"expired"};case"page_cancelled":return{kind:"cancelled"};case"abandoned":return{kind:m?"timed_out":"cancelled"}}},n,I,f,r,h,t,F,d,v),p?.importToolName),p?.statusToolName):$[41];return J3($,42,L,te,b)?K3($,42,L,te,b,{state:b,send:te,runForTool:L,dismiss:k}):$[45]}function hr(e){let $=J9(30),{showable:t,boot:n,sendToAppsAdminOff:r,destinations:i,offered:a,conversationPending:s,host:c,src:d}=e,f=s!==void 0&&s,p=F(),m=S(),h=i.destinations,g=i.waiting??gr,_=J2($,0,n,f)||J1($,2,i.needsMp4)||J5($,3,i.status,h,a,r,t,g)?I5($,3,Z1($,2,Z2($,0,t?be({bootStatus:n.status,orgIsActive:n.orgIsActive,hasHandoffToken:n.handoffToken!==void 0,connectorsAdminOff:i.status==="admin_off",sendToAppsAdminOff:r,connectorsStoreFailed:i.status==="store_error",directoryFailed:i.status==="directory_error",browserCannotMakeMp4:i.needsMp4,pageNotReady:a&&i.status==="ready"&&h?.some(_r)===!1&&g.some(_r),conversationPending:f}):null,n,f),i.needsMp4),i.status,h,a,r,t,g):$[9],[v,y]=u(null),b=i.status==="loading"||i.status==="store_error"||i.status==="directory_error",x=_==="connectors_unavailable"||_==="directory_unavailable"||_==="conversation_pending"?_:_===null&&t&&b?v:null;x!==v&&y(x);let C=_??x,w=J2($,10,p,C)?K2($,10,p,C,xe(p,C)):$[12],T=o(null),E=w?.reason,D,O;J4($,13,m,c,E,d)?(D=()=>{let e=E===void 0?null:`${d}|${E}`;e!==null&&e!==T.current&&(T.current=e,m.track({event_key:"claudeai.jolly_candle.unavailable",reason:E,host:c}))},O=[E,d,c,m],K5($,13,m,c,E,d,D,O)):(D=$[17],O=$[18]),l(D,O);let k=i.retry,A=J2($,19,k,C)?K2($,19,k,C,()=>{C!==null&&Se[C]==="retry"&&k()}):$[21],j=J3($,22,h,w,g)?K3($,22,h,w,g,w===void 0||g.length===0?h:[...h??[],...g]):$[25];return J3($,26,A,j,w)?K3($,26,A,j,w,{unavailable:w,retry:A,rows:j}):$[29]}var gr=[];function _r(e){return e.status===void 0}function vr(e,t,n){return e&&(!t.orgIsActive||t.handoffToken===void 0||n.sendToAppsAdminOff||n.conversationPending===!0)}export{qe as a,He as c,On as i,Fe as l,hr as n,tt as o,mr as r,Ie as s,vr as t,Pe as u};