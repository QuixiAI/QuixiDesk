import{Dp as e,hp as t}from"./shared-104-CTONUSaz.js";import{at as n,ot as r,pt as i,st as a}from"./shared-156-BLTosN8U.js";var o=/\r\n|[\r\n\u0085\u2028\u2029]/g,s=new RegExp(n.source,"g"),c=/[\s\p{Zs}]+/gu;function l(e){return h(e).replace(c," ").trim()}function u(n){return t(e(n))}function d(n){let r=e(n);return t(r)!==r}function f(e){return l(u(e))}function p(e){return m(u(e))}function m(e){return e.replace(o,`
`).split(/\n(?:[^\S\n]*\n)+/).map(h).filter(e=>e!=="").join(`

`)}function h(e){return i(e.replace(o,`
`)).replace(s,a).split(`
`).map(e=>r().test(e)?"":e).join(`
`).replace(/\n{2,}/g,`
`).trim()}export{f as a,h as i,l as n,p as o,m as r,d as t};