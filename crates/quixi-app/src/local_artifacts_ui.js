;(function () {
  var dialog, previousFocus, shownRoute = '', loading = false;
  function closePreview(returnToGallery) {
    if (!dialog) return;
    dialog.remove(); dialog = null;
    if (previousFocus && previousFocus.isConnected) previousFocus.focus();
    if (returnToGallery !== false && /^\/cowork-artifact\/[^/]+/.test(location.pathname)) {
      history.pushState({}, '', '/cowork-artifact');
      window.dispatchEvent(new PopStateEvent('popstate'));
    }
  }
  function showPreview(id) {
    if (loading) return;
    loading = true;
    fetch('/api/quixi/artifacts/' + encodeURIComponent(id)).then(function (response) {
      if (!response.ok) throw new Error('Preview unavailable');
      return response.json();
    }).then(function (artifact) {
      closePreview(false); previousFocus = document.activeElement;
      dialog = document.createElement('dialog');
      dialog.id = 'quixi-local-artifact-preview';
      dialog.style.cssText = 'position:fixed;inset:0;margin:auto;width:min(1000px,90vw);max-width:90vw;height:85vh;max-height:85vh;padding:0;border:1px solid #334155;border-radius:14px;background:#0f172a;color:#f8fafc;';
      dialog.setAttribute('aria-label', artifact.name + ' — local preview');
      var header = document.createElement('div');
      header.style.cssText = 'display:flex;align-items:center;gap:16px;padding:16px;';
      var heading = document.createElement('h2');
      heading.textContent = artifact.name; heading.style.cssText = 'flex:1;margin:0;font-size:18px;';
      var privacy = document.createElement('span'); privacy.textContent = 'Private local preview';
      var close = document.createElement('button'); close.textContent = 'Close';
      close.style.cssText = 'padding:8px 16px;border-radius:6px;background:#334155;color:white;border:0;cursor:pointer;';
      close.onclick = closePreview;
      header.append(heading, privacy, close);
      var frame = document.createElement('iframe');
      frame.title = artifact.name;
      frame.setAttribute('sandbox', 'allow-scripts');
      frame.style.cssText = 'display:block;width:100%;height:calc(100% - 70px);border:0;background:white;';
      // This is untrusted model output. Scripts, forms and external requests stay disabled.
      // Only our nonce-bearing keyboard bridge can run; artifact scripts stay blocked.
      var nonce = Array.from(crypto.getRandomValues(new Uint8Array(24)), function (b) { return b.toString(16).padStart(2, '0'); }).join('');
      frame.srcdoc = '<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; script-src \'nonce-' + nonce + '\'; style-src \'unsafe-inline\'; img-src data:; font-src data:; form-action \'none\'; base-uri \'none\'">' +
        '<script nonce="' + nonce + '">document.addEventListener("keydown",function(e){if(e.key==="Escape"){e.preventDefault();parent.postMessage({type:"quixi-artifact-escape"},"*");}},true);</script>' + artifact.html;
      dialog.append(header, frame); document.body.appendChild(dialog);
      dialog.addEventListener('cancel', function (event) { event.preventDefault(); closePreview(); });
      dialog.showModal(); close.focus();
    }).catch(function (error) {
      console.error('Local artifact preview:', error.message);
    }).finally(function () { loading = false; });
  }
  function checkRoute() {
    var route = location.pathname;
    if (route === shownRoute) return;
    shownRoute = route;
    closePreview(false);
    var match = /^\/cowork-artifact\/([0-9a-f-]+)\/?$/.exec(route);
    if (match) showPreview(match[1]);
  }
  function install() {
    if (window.__QUIXI_LISTENERS__) {
      window.__QUIXI_LISTENERS__.set('quixi:artifact-preview', new Set([function (_event, data) {
        if (data && data.id) showPreview(data.id);
      }]));
    }
    window.addEventListener('keydown', function (event) {
      if (dialog && event.key === 'Escape') {
        event.preventDefault(); event.stopImmediatePropagation(); closePreview();
      }
    }, true);
    window.addEventListener('message', function (event) {
      var frame = dialog && dialog.querySelector('iframe');
      if (frame && event.source === frame.contentWindow && event.data && event.data.type === 'quixi-artifact-escape') closePreview();
    });
    window.addEventListener('popstate', checkRoute);
    new MutationObserver(checkRoute).observe(document.body, { childList: true, subtree: true });
    checkRoute();
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', install);
  else install();
})();
