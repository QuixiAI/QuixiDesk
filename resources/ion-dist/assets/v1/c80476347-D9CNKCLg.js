import{n as e}from"./preload-helper-D0dM6GpB.js";import{lp as t}from"./shared-25-DOWZRSlq.js";import{Ba as n}from"./shared-0-DDOOdL6P.js";import{Pt as r}from"./shared-104-CTONUSaz.js";import{t as i}from"./c36273108-nfRg0EqW.js";async function a(e,t=1e3){let r=e.querySelectorAll('link[rel="stylesheet"]');if(r.length===0)return;let i=Array.from(r).map(e=>new Promise(t=>{if(e.sheet){t();return}e.addEventListener("load",()=>t()),e.addEventListener("error",()=>t())}));await Promise.race([Promise.all(i),n(t)])}function o(e){let t=[e,...e.querySelectorAll("*")];if(!t.some(e=>e.shadowRoot))return e.outerHTML;let n=e.cloneNode(!0),r=[n,...n.querySelectorAll("*")];return t.forEach((e,t)=>{let n=r[t];e.shadowRoot&&n&&!e.hasChildNodes()&&(n.innerHTML=e.shadowRoot.innerHTML)}),n.outerHTML}var s=6e4,c=new WeakMap;function l(e,t){let n=Symbol("print"),r=c.get(e)?.original??e.title;c.set(e,{original:r,owner:n}),e.title=t;let i=e.title;return()=>{c.get(e)?.owner===n&&(c.delete(e),e.title===i&&(e.title=r))}}async function u({title:n,element:r,doc:c=document,landscape:u=!1}){return new Promise((d,f)=>{try{let p=c.head.innerHTML,m=Array.from(c.body.querySelectorAll('link[rel="stylesheet"]'),e=>e.outerHTML).join(`
`),h=c.createElement("iframe");h.style.position="fixed",h.style.right="0",h.style.bottom="0",h.style.width="0",h.style.height="0",h.style.border="0",h.setAttribute("sandbox","allow-same-origin allow-modals"),h.setAttribute("csp","");let g=!1;h.onload=()=>{g=!0},c.body.appendChild(h);let _=h.contentDocument||h.contentWindow?.document;if(!_){c.body.removeChild(h),f(e(Error("Could not access iframe document"),"Could not access iframe document","literal"));return}_.write(`
        <!DOCTYPE html>
        <html>
          <head>
            <title>${i(n)}</title>
            ${p}
            <style>
              /* Remove browser-added headers and footers */
              @page {
                size: ${u?"landscape":"auto"};
                margin: 0; /* Keep at 0 to remove datetime, page count, website, etc */
                padding: 12mm 0; /* Per-page top/bottom inset; Chromium-only. Sides come from body. */
              }
              @media print {
                body {
                  margin: 0;
                  padding: 0 15mm;
                }
                /* The panels' on-screen side inset and reading width; on paper
                   the body padding above stands in for both. */
                #markdown-artifact, #wiggle-file-content {
                  max-width: none;
                  margin-inline: 0;
                  padding-inline: 0;
                }
                html, body, pre, code {
                  -webkit-print-color-adjust: exact !important;
                  color-adjust: exact !important;
                  print-color-adjust: exact !important;
                }
              
                /* Smart page breaks */
                h1, h2, h3 {
                  page-break-after: avoid !important;
                }
                img, pre, table {
                  page-break-inside: avoid !important;
                }
              }
            </style>
          </head>
          <body>
            ${o(r)}
            ${m}
          </body>
        </html>
      `),_.close();for(let e of["lang","data-cjk-order","data-theme","data-density"]){let t=c.documentElement.getAttribute(e);t!==null&&_.documentElement.setAttribute(e,t)}c.documentElement.classList.contains("cds-root")&&_.documentElement.classList.add("cds-root"),_.documentElement.setAttribute("data-print-document",r.closest("[data-mode]")?.getAttribute("data-mode")??"");let v=t(c),y,b=!1,x=()=>{try{y=l(c,n),h.contentWindow?.addEventListener("afterprint",()=>{v.setTimeout(S,100)});try{h.contentWindow?.print()}catch{h.contentDocument?.execCommand("print")}v.setTimeout(d,100),v.setTimeout(S,s)}catch(e){S(),f(e)}},S=()=>{if(!b){b=!0;try{y?.(),c.body.removeChild(h)}catch{}}};(async()=>{try{g||await new Promise((e,t)=>{h.onload=()=>e(),h.onerror=e=>t(e)}),await a(_),x()}catch(e){S(),f(e)}})()}catch(e){f(e)}})}async function d(t,n=document){if(t.type!==r.Markdown)throw e(Error("Only markdown artifacts can be saved as PDF"),"Only markdown artifacts can be saved as PDF","literal");let i=n.getElementById("markdown-artifact");if(!i)throw e(Error("Markdown artifact element not found"),"Markdown artifact element not found","literal");return u({title:t?.title||"Artifact",element:i,doc:n})}function f(e){let t=document.createElement("pre");return t.style.whiteSpace="pre-wrap",t.style.wordBreak="break-word",t.textContent=e,t}async function p(t,n,r=document){let i=t.path.split("/").pop()||t.path;if(n!==void 0)return u({title:i,element:f(n),doc:r});let a=r.getElementById("wiggle-file-content");if(!a)throw e(Error("File content element not found"),"File content element not found","literal");return u({title:i,element:a,doc:r})}export{d as n,p as r,u as t};