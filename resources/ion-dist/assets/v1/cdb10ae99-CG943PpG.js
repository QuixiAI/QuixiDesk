import{mp as e}from"./shared-130-dWztbTO1.js";import{d as t,l as n}from"./cb5bb65ab-CcLuCCiC.js";function r(r,i=e){let a=t(r);if(!a)return i(r);let o=n(r,a,e),s=i(r.slice(0,o.start));return s+(s.endsWith(`
`)||o.text===""||o.text.startsWith(`
`)?"":`
`)+o.text+i(r.slice(o.end))}export{r as t};