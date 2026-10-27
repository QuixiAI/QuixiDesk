import{fm as e}from"./shared-0-DDOOdL6P.js";import{Hf as t,tp as n}from"./shared-52-uHFNjl7C.js";import{Ed as r}from"./shared-182-BI1KJccu.js";var i=8e7,a=1048576,o=65536,s=15e3,c=1e4,l=2,u=6e5;function d(e){return e>=192&&e<=207&&e!==196&&e!==200&&e!==204}async function f(e){let t=Math.min(e.size,a),n=new Uint8Array(await e.slice(0,Math.min(t,o)).arrayBuffer()),r=async r=>{if(n.length>=t||r>t)return!1;let i=Math.min(t,Math.max(r,n.length*2));return n=new Uint8Array(await e.slice(0,i).arrayBuffer()),r<=n.length},i=async e=>e<=n.length||r(e);if(!await i(2)||n[0]!==255||n[1]!==216)return null;let s=2;for(;;){if(s+4>n.length&&!await i(s+4)||n[s]!==255)return null;let e=n[s+1];if(e===255){s+=1;continue}if(e===1||e>=208&&e<=216){s+=2;continue}if(e===217||e===218)return null;let t=n[s+2]<<8|n[s+3];if(t<2)return null;if(d(e)){if(!await i(s+9))return null;let e=n[s+5]<<8|n[s+6],t=n[s+7]<<8|n[s+8];return t>0&&e>0?{width:t,height:e}:null}s+=2+t}}var p="/9j/4AAQSkZJRgABAQAAAQABAAD/4QAiRXhpZgAATU0AKgAAAAgAAQESAAMAAAABAAYAAAAAAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////2wBDAf//////////////////////////////////////////////////////////////////////////////////////wAARCAABAAIDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAP/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/8QAFAEBAAAAAAAAAAAAAAAAAAAAAP/EABQRAQAAAAAAAAAAAAAAAAAAAAD/2gAMAwEAAhEDEQA/AKAA/9k=",m=String.raw`
let orientationApplied;
async function appliesExifOrientation(probeBase64) {
  if (orientationApplied === undefined) {
    try {
      const bytes = Uint8Array.from(atob(probeBase64), (c) => c.charCodeAt(0));
      const probe = await createImageBitmap(new Blob([bytes], { type: "image/jpeg" }));
      orientationApplied = probe.width === 1 && probe.height === 2;
      probe.close();
    } catch {
      orientationApplied = null;
    }
  }
  return orientationApplied;
}
self.onmessage = async (event) => {
  const { id } = event.data || {};
  try {
    const { file, maxLongEdgePx, quality, probeBase64 } = event.data;
    const upright = await appliesExifOrientation(probeBase64);
    if (!upright) {
      self.postMessage({
        id,
        error: upright === null ? "OrientationProbeFailed" : "ExifOrientationUnsupported",
      });
      return;
    }
    const bitmap = await createImageBitmap(file);
    let blob;
    let width;
    let height;
    try {
      const scale = Math.min(1, maxLongEdgePx / Math.max(bitmap.width, bitmap.height));
      width = Math.max(1, Math.round(bitmap.width * scale));
      height = Math.max(1, Math.round(bitmap.height * scale));
      const canvas = new OffscreenCanvas(width, height);
      const context = canvas.getContext("2d");
      if (!context) {
        throw new Error("no 2d context");
      }
      context.imageSmoothingEnabled = true;
      context.imageSmoothingQuality = "high";
      context.fillStyle = "#fff";
      context.fillRect(0, 0, width, height);
      context.drawImage(bitmap, 0, 0, width, height);
      // Not redundant with the finally: closing before the encode lowers the worker's peak memory.
      bitmap.close();
      blob = await canvas.convertToBlob({ type: "image/jpeg", quality });
      canvas.width = 0;
      canvas.height = 0;
    } finally {
      bitmap.close();
    }
    self.postMessage({ id, blob, width, height });
  } catch (error) {
    self.postMessage({ id, error: String((error && error.name) || "Error") });
  }
};
`,h,g=0,_=!1,v,y=Promise.resolve(),b=0,x=1,S=0;function C(){return typeof Worker<"u"&&typeof createImageBitmap<"u"&&typeof OffscreenCanvas<"u"&&"convertToBlob"in OffscreenCanvas.prototype&&typeof OffscreenCanvasRenderingContext2D<"u"&&"imageSmoothingQuality"in OffscreenCanvasRenderingContext2D.prototype&&typeof URL<"u"&&typeof URL.createObjectURL=="function"}function w(){if(h===null&&g!==0&&Date.now()>=g&&(h=void 0,b=0),h!==void 0)return h;try{v??=URL.createObjectURL(new Blob([m],{type:"text/javascript"}));let e=new Worker(v,{name:"photo-downscale"});e.addEventListener("error",t=>{t.preventDefault(),E(e)}),h=e}catch{T()}return h??null}function T(){h=null,g=Date.now()+u}function E(e){e.terminate(),h===e&&(b+=1,b>=l?T():h=void 0)}function D(e){return e<=2e7?"running_upto_20mp":e<=4e7?"running_upto_40mp":"running_over_40mp"}function O(t,n,r,i,a,o,c){return new Promise(l=>{let u=(n,r=!1)=>{clearTimeout(g),t.removeEventListener("message",f),t.removeEventListener("error",m),c?.removeEventListener("abort",d);let i=n.error===void 0?"finished":"failed";e("photo_downscale_job",r?i:"abandoned"),l(n)},d=()=>{t.terminate(),h===t&&(h=void 0),u({id:n,error:"cancelled"})},f=e=>{e.data?.id===n&&(e.data.error==="OrientationProbeFailed"?E(t):b=0,e.data.error==="ExifOrientationUnsupported"&&(_=!0,t.terminate(),h===t&&(h=void 0)),u(e.data,!0))},m=()=>{E(t),u({id:n,error:"worker_crashed"})},g=setTimeout(()=>{E(t),u({id:n,error:"timed_out"})},s);t.addEventListener("message",f),t.addEventListener("error",m),c?.addEventListener("abort",d,{once:!0}),e("photo_downscale_job",D(i));try{t.postMessage({id:n,file:r,maxLongEdgePx:a,quality:o,probeBase64:p})}catch{u({id:n,error:"worker_crashed"})}})}function k(e,t,n,r,i){let a=x++;if(i?.aborted)return Promise.resolve({id:a,error:"cancelled"});let o,s=new Promise(e=>{o=e}),l=!0,u=e=>l?(l=!1,clearTimeout(d),i?.removeEventListener("abort",f),e!==void 0&&o({id:a,error:e}),!0):!1,d=setTimeout(()=>u("timed_out"),c),f=()=>{u("cancelled")};i?.addEventListener("abort",f,{once:!0});let p=async()=>{if(!u())return;if(i?.aborted){o({id:a,error:"cancelled"});return}if(_){o({id:a,error:"ExifOrientationUnsupported"});return}let s=w();if(!s){o({id:a,error:"worker_unavailable"});return}o(await O(s,a,e,t,n,r,i))};S+=1;let m=()=>{--S};return y=y.then(p,p).then(m,m),s}function A(e){switch(e){case"cancelled":return"cancelled";case"worker_unavailable":return"worker_unavailable";case"worker_crashed":return"worker_crashed";case"timed_out":return"timed_out";case"ExifOrientationUnsupported":case"OrientationProbeFailed":return"engine_unsupported";default:return"reencode_failed"}}function j(e){if(n.includes(r(e)))return e;let t=e.lastIndexOf(".");return`${t>0?e.slice(0,t):e}.jpg`}async function M(e,n){let r;try{r=await f(e)}catch{r=null}if(!r)return{file:e,outcome:"no_jpeg_header"};let a=Math.max(r.width,r.height);if(a<=n.maxLongEdgePx)return{file:e,outcome:"within_cap",sourceLongEdgePx:a};if(r.width*r.height>i)return{file:e,outcome:"too_many_pixels",sourceLongEdgePx:a};if(_||!C())return{file:e,outcome:"engine_unsupported",sourceLongEdgePx:a};let o=await k(e,r.width*r.height,n.maxLongEdgePx,n.quality,n.signal);return!o.blob||o.blob.size===0||o.blob.type!=="image/jpeg"?{file:e,outcome:A(o.error),sourceLongEdgePx:a}:o.blob.size>=e.size?{file:e,outcome:"not_smaller",sourceLongEdgePx:a}:{file:new File([o.blob],j(e.name),{type:t,lastModified:e.lastModified}),outcome:"resized",sourceLongEdgePx:a}}var N={reset(){h?.terminate(),h=void 0,g=0,y=Promise.resolve(),b=0,_=!1,S=0},readJpegDimensions:f,jobsInFlight:()=>S,drain:()=>y,PHOTO_ENCODE_WORKER_SOURCE:m,MAX_DECODE_PIXELS:i,MAX_QUEUE_WAIT_MS:c,MAX_WORKER_FAILURES:l,WORKER_JOB_TIMEOUT_MS:s,WORKER_RETRY_AFTER_MS:u};export{N as _test,M as downscalePhotoForUpload};