;(function () {
  var notice, status = {issues: []};
  function show(next) {
    status = next || {issues: []};
    if (!document.body) return;
    if (notice) notice.remove();
    notice = null;
    if (!status.issues || !status.issues.length) return;
    var unsaved = status.issues.some(function (issue) { return !issue.recoverySaved; });
    notice = document.createElement('section');
    notice.id = 'quixi-storage-warning';
    notice.setAttribute('role', 'alert');
    notice.setAttribute('aria-label', 'Conversation storage problem');
    notice.style.cssText = 'position:fixed;z-index:2147483647;right:16px;top:48px;box-sizing:border-box;width:min(440px,calc(100vw - 32px));max-height:calc(100vh - 64px);overflow:auto;padding:16px;border:2px solid #fbbf24;border-radius:10px;background:#422006;color:#fff7ed;font:14px/1.4 system-ui;box-shadow:0 8px 32px #0008;';
    var title = document.createElement('strong');
    title.textContent = unsaved ? 'Conversation not saved — keep QuixiDesk open' : 'Conversation saved in recovery storage';
    var description = document.createElement('p');
    description.textContent = unsaved
      ? 'QuixiDesk could not save your conversation or a recovery copy. It is still in memory. Restore storage access or free disk space, then retry saving. Quitting is blocked while your only copy is in memory.'
      : 'Your usual conversation folder is unavailable. A recovery copy will restore your conversation after restart. Restore storage access, then retry saving.';
    var details = document.createElement('details');
    details.style.cssText = 'margin:8px 0;';
    var summary = document.createElement('summary');
    summary.textContent = 'Storage error details (' + status.issues.length + ')';
    var errors = document.createElement('p');
    errors.textContent = status.issues.map(function (issue) { return issue.title + ': ' + issue.message; }).join(' · ');
    details.append(summary, errors);
    var retry = document.createElement('button');
    retry.textContent = 'Retry saving';
    retry.onclick = function () {
      retry.disabled = true;
      fetch('/api/quixi/storage/retry', {method: 'POST'}).then(function (response) {
        if (!response.ok) throw new Error('Storage retry failed');
        return response.json();
      }).then(show).catch(function () { retry.disabled = false; description.textContent = 'Saving could not be retried. Keep QuixiDesk open and try again.'; });
    };
    var copy = document.createElement('button');
    copy.textContent = 'Copy recovery data';
    copy.onclick = function () {
      fetch('/api/quixi/storage/export').then(function (response) {
        if (!response.ok) throw new Error('Export failed');
        return response.json();
      }).then(function (data) {
        return fetch('app://localhost/__eipc__/clipboard', {method: 'POST', headers: {'Content-Type':'application/json'},
          body: JSON.stringify({action:'write', text:JSON.stringify(data, null, 2)})});
      }).then(function (response) {
        if (!response.ok) throw new Error('Copy failed');
        copy.textContent = 'Copied — paste into a file to keep a backup';
      }).catch(function () { copy.textContent = 'Copy failed — try again'; });
    };
    [retry, copy].forEach(function (button) {
      button.style.cssText = 'padding:8px 12px;margin:0 10px 0 0;border:1px solid #fbbf24;border-radius:6px;background:#713f12;color:white;cursor:pointer;font:inherit;';
    });
    notice.append(title, description, details, retry, copy);
    document.body.appendChild(notice);
  }
  window.__quixi_show_storage_status__ = show;
  function install() {
    if (window.__QUIXI_LISTENERS__) {
      if (!window.__QUIXI_LISTENERS__.has('quixi:storage-status')) window.__QUIXI_LISTENERS__.set('quixi:storage-status', new Set());
      window.__QUIXI_LISTENERS__.get('quixi:storage-status').add(function (_event, next) { show(next); });
    }
    fetch('/api/quixi/storage/status').then(function (response) { return response.json(); }).then(show).catch(function () {});
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', install);
  else install();
})();
