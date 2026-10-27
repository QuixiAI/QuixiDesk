var e=/(?:^|^)\s*(?:export\s+)?([\w.-]+)(?:\s*=\s*?|:\s+?)(\s*'(?:\\'|[^'])*'|\s*"(?:\\"|[^"])*"|\s*`(?:\\`|[^`])*`|[^#\r\n]+)?\s*(?:#.*)?(?:$|$)/gm;function t(t){let n=Object.create(null),r=t.replace(/\r\n?/gm,`
`),i;for(;(i=e.exec(r))!==null;){let e=i[1],t=i[2]??"";t=t.trim();let r=t[0];t=t.replace(/^(['"`])([\s\S]*)\1$/gm,"$2"),r==='"'&&(t=t.replace(/\\n/g,`
`),t=t.replace(/\\r/g,"\r")),n[e]=t}return n}function n(t,n){if(!t.trim())return;let r=t.replace(/\r\n?/g,`
`).replace(e,"").split(`
`);for(let e=0;e<r.length;e++){let t=r[e].trim();if(!(!t||t.startsWith("#"))&&!(n?.allowTrailingBareKey&&e===r.length-1&&/^(?:export\s+)?[\w.-]+:?$/.test(t)))return t}}export{t as n,n as t};