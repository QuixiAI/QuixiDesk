import{Co as e,Ha as t,ho as n,so as r}from"./vendor-frame-CnSY1sxv.js";import{$f as J2,Qf as J1,Zf as J0,ap as K0,ip as J9,lp as K4,np as J5,op as K1,rp as J8,sp as K2,tp as J4,up as K5}from"./shared-frame-6jtSkIit.js";import{Cg as i,dg as a,sg as o,xg as s}from"./shared-2-TF8v67kg.js";var c=0,l=.3,u=.3,d=4/3,f=1,p=1,m=.25,h=.15,g=.2,_=.12,v=.8,y=1.1,b=.04,x=.005,S=.07,C=.006,w=.13,T=.015,E=.11,D=.02,O=.15,k=24,A=50,j=150,M=300,N=.5,P=.3,F=.2,I=.75,L=3,R=.6,z=3500,B=1,V=.6,H=.05,ee=1,te=.1,U=.25,W=[2,4,8,12,16,20,24,28,32,36,40].map(e=>e*U);function G(){return`
  attribute vec2 aVertexPosition;
  uniform mat4 uModelViewMatrix;
  varying vec2 vPosition;

  void main(void) {
    vec2 normalizedPosition = aVertexPosition / ${l.toFixed(2)};
    gl_Position = uModelViewMatrix * vec4(aVertexPosition, 0.0, 1.0);
    vPosition = normalizedPosition;
  }
`}function K(){return`
  precision highp float;
  varying vec2 vPosition;
  uniform vec4 uBaseColor;
  uniform float uOpacityFactor;
  uniform float uOvalRatio;

  void main(void) {
    vec2 ovalPos = vec2(vPosition.x / uOvalRatio, vPosition.y);
    float dist = length(ovalPos);

    // Hard edge with slight anti-aliasing at boundary
    float alpha = 1.0 - smoothstep(0.95, 1.0, dist);
    alpha *= ${R.toFixed(2)};
    alpha *= uBaseColor.a * uOpacityFactor;

    vec3 premultColor = uBaseColor.rgb * alpha;
    gl_FragColor = vec4(premultColor, alpha);
  }
`}function ne(){return`
  attribute vec2 aVertexPosition;
  attribute vec2 aTexCoord;
  varying vec2 vTexCoord;

  void main(void) {
    gl_Position = vec4(aVertexPosition, 0.0, 1.0);
    vTexCoord = aTexCoord;
  }
`}function re(){return`
  precision highp float;
  varying vec2 vTexCoord;
  uniform sampler2D uTexture;
  uniform vec2 uResolution;
  uniform float uOffset; // Kawase offset in texels of the sampled texture
  uniform float uTime;
  uniform float uIsLastPass; // 1.0 for final pass (apply noise), 0.0 otherwise

  float random(vec2 st) {
    return fract(sin(dot(st.xy, vec2(12.9898, 78.233))) * 43758.5453123);
  }

  void main(void) {
    vec2 texelSize = 1.0 / uResolution;

    // Kawase blur: sample 4 diagonal corners + center
    vec4 result = texture2D(uTexture, vTexCoord) * 4.0;
    result += texture2D(uTexture, vTexCoord + vec2(-uOffset, -uOffset) * texelSize);
    result += texture2D(uTexture, vTexCoord + vec2(-uOffset,  uOffset) * texelSize);
    result += texture2D(uTexture, vTexCoord + vec2( uOffset, -uOffset) * texelSize);
    result += texture2D(uTexture, vTexCoord + vec2( uOffset,  uOffset) * texelSize);
    vec4 blurred = result / 8.0;

    // Apply noise on final pass only
    ${`
    if (uIsLastPass > 0.5) {
      vec2 noiseCoord = gl_FragCoord.xy * 0.01;
      float animSpeed = ${k.toFixed(1)};
      float timeScale = floor(uTime * animSpeed) / animSpeed;

      float noise = 0.0;
      vec2 p1 = floor(noiseCoord * ${A.toFixed(1)}) + vec2(timeScale * 1.0, timeScale * 0.7);
      noise += random(p1) * ${N.toFixed(1)};
      vec2 p2 = floor(noiseCoord * ${j.toFixed(1)}) + vec2(timeScale * 0.5, timeScale * 1.3);
      noise += random(p2) * ${P.toFixed(1)};
      vec2 p3 = floor(noiseCoord * ${M.toFixed(1)}) + vec2(timeScale * 1.4, timeScale * 0.2);
      noise += random(p3) * ${F.toFixed(1)};

      float grainThreshold = ${I.toFixed(2)};
      float grainIntensity = ${O.toFixed(4)} * ${L.toFixed(1)};
      float grainMask = smoothstep(grainThreshold, 0.99, noise);
      vec3 grainColor = vec3(1.0) * grainMask * grainIntensity;

      // Only add noise where there's content (based on alpha)
      blurred.rgb += grainColor * blurred.a;
    }
    `}

    gl_FragColor = blurred;
  }
`}function q(e,t,n){let r=e.createFramebuffer(),i=e.createTexture();return!r||!i?{framebuffer:null,texture:null}:(e.bindTexture(e.TEXTURE_2D,i),e.texImage2D(e.TEXTURE_2D,0,e.RGBA,t,n,0,e.RGBA,e.UNSIGNED_BYTE,null),e.texParameteri(e.TEXTURE_2D,e.TEXTURE_MIN_FILTER,e.LINEAR),e.texParameteri(e.TEXTURE_2D,e.TEXTURE_MAG_FILTER,e.LINEAR),e.texParameteri(e.TEXTURE_2D,e.TEXTURE_WRAP_S,e.CLAMP_TO_EDGE),e.texParameteri(e.TEXTURE_2D,e.TEXTURE_WRAP_T,e.CLAMP_TO_EDGE),e.bindFramebuffer(e.FRAMEBUFFER,r),e.framebufferTexture2D(e.FRAMEBUFFER,e.COLOR_ATTACHMENT0,e.TEXTURE_2D,i,0),e.bindFramebuffer(e.FRAMEBUFFER,null),e.bindTexture(e.TEXTURE_2D,null),{framebuffer:r,texture:i})}function ie(e){let t=e.createBuffer();e.bindBuffer(e.ARRAY_BUFFER,t),e.bufferData(e.ARRAY_BUFFER,new Float32Array([-1,-1,1,-1,1,1,-1,-1,1,1,-1,1]),e.STATIC_DRAW);let n=e.createBuffer();return e.bindBuffer(e.ARRAY_BUFFER,n),e.bufferData(e.ARRAY_BUFFER,new Float32Array([0,0,1,0,1,1,0,0,1,1,0,1]),e.STATIC_DRAW),{position:t,texCoord:n,vertexCount:6}}function ae(e){let t=Y(e,e.VERTEX_SHADER,ne()),n=Y(e,e.FRAGMENT_SHADER,re());if(!t||!n)return null;let r=X(e,t,n);return r?{program:r,attribLocations:{vertexPosition:e.getAttribLocation(r,"aVertexPosition"),texCoord:e.getAttribLocation(r,"aTexCoord")},uniformLocations:{texture:e.getUniformLocation(r,"uTexture"),resolution:e.getUniformLocation(r,"uResolution"),offset:e.getUniformLocation(r,"uOffset"),time:e.getUniformLocation(r,"uTime"),isLastPass:e.getUniformLocation(r,"uIsLastPass")}}:null}function J(e){let t=e.match(/rgba\((\d+),\s*(\d+),\s*(\d+),\s*([0-9.]+)\)/);return t?[parseInt(t[1])/255,parseInt(t[2])/255,parseInt(t[3])/255,parseFloat(t[4])]:[0,0,0,0]}function oe(e,t,n){return[e[0]+(t[0]-e[0])*n,e[1]+(t[1]-e[1])*n,e[2]+(t[2]-e[2])*n,e[3]+(t[3]-e[3])*n]}function se(e,t,n,r){let i=t.context.sampleRate/2,a=Math.floor(n/i*(e.length-1)),o=Math.floor(r/i*(e.length-1)),s=0;for(let t=a;t<=o;t++)s+=e[t];return s/((o-a+1)*255)}function Y(e,t,n){let r=e.createShader(t);return r?(e.shaderSource(r,n),e.compileShader(r),e.getShaderParameter(r,e.COMPILE_STATUS)?r:(e.deleteShader(r),null)):null}function X(e,t,n){let r=e.createProgram();return!r||(e.attachShader(r,t),e.attachShader(r,n),e.linkProgram(r),!e.getProgramParameter(r,e.LINK_STATUS))?null:r}function ce(e){let t=e.createBuffer();e.bindBuffer(e.ARRAY_BUFFER,t);let n=l,r=n,i=[-.3,-.3,n,-.3,n,r,-.3,-.3,n,r,-.3,r];return e.bufferData(e.ARRAY_BUFFER,new Float32Array(i),e.STATIC_DRAW),{position:t,vertexCount:6}}var le=/swiftshader|llvmpipe|softpipe|basic render driver|software/i;function Z(e){try{let t=e.getExtension("WEBGL_debug_renderer_info"),n=e.getParameter(t?t.UNMASKED_RENDERER_WEBGL:e.RENDERER);return typeof n=="string"&&le.test(n)}catch{return!1}}var Q=null;function ue(e,t,n,r,i={}){if(!e)return;let a=t=>{e.getAttachedShaders(t)?.forEach(t=>{e.deleteShader(t)}),e.deleteProgram(t)};try{r&&(e.deleteFramebuffer(r.framebuffer1),e.deleteFramebuffer(r.framebuffer2),e.deleteTexture(r.texture1),e.deleteTexture(r.texture2),e.deleteBuffer(r.quadBuffers.position),e.deleteBuffer(r.quadBuffers.texCoord),a(r.blurProgram.program)),n&&e.deleteBuffer(n.position),t&&a(t.program)}catch{}if(i.releaseContext)try{e.getExtension("WEBGL_lose_context")?.loseContext()}catch{}}function de(){if(Q!==null)return Q;if(typeof document>"u")return!1;let e=null;try{if(e=document.createElement("canvas").getContext("webgl",{depth:!1,stencil:!1}),e===null)return!1;Q=!Z(e)}catch{Q=!1}finally{try{e?.getExtension("WEBGL_lose_context")?.loseContext()}catch{}}return Q??!1}function fe(e){let t=e.getContext("webgl",{alpha:!0,premultipliedAlpha:!0,antialias:!0,depth:!1,stencil:!1});if(!t||Z(t))return{gl:null,programInfo:null,buffers:null,blurResources:null};t.enable(t.BLEND),t.blendEquation(t.FUNC_ADD),t.blendFunc(t.ONE,t.ONE_MINUS_SRC_ALPHA),t.disable(t.DEPTH_TEST),t.clearColor(0,0,0,0);let n=Y(t,t.VERTEX_SHADER,G()),r=Y(t,t.FRAGMENT_SHADER,K());if(!n||!r)return{gl:t,programInfo:null,buffers:null,blurResources:null};let i=X(t,n,r);if(!i)return{gl:t,programInfo:null,buffers:null,blurResources:null};let a={program:i,attribLocations:{vertexPosition:t.getAttribLocation(i,"aVertexPosition")},uniformLocations:{baseColor:t.getUniformLocation(i,"uBaseColor"),modelViewMatrix:t.getUniformLocation(i,"uModelViewMatrix"),opacityFactor:t.getUniformLocation(i,"uOpacityFactor"),ovalRatio:t.getUniformLocation(i,"uOvalRatio")}},o=ce(t),s=null,c=ae(t),l=ie(t),u=Math.max(1,Math.round(e.width*U)),d=Math.max(1,Math.round(e.height*U)),f=q(t,u,d),p=q(t,u,d);return c&&f.framebuffer&&f.texture&&p.framebuffer&&p.texture&&(s={blurProgram:c,framebuffer1:f.framebuffer,texture1:f.texture,framebuffer2:p.framebuffer,texture2:p.texture,width:u,height:d,quadBuffers:l}),{gl:t,programInfo:a,buffers:o,blurResources:s}}function pe(e,t,n,r,i,a,o,s){let l=(o?.circleBaseY??c)+(o?.yShiftClip??0),O=i/1e3;s?(e.bindFramebuffer(e.FRAMEBUFFER,s.framebuffer1),e.viewport(0,0,s.width,s.height)):e.viewport(0,0,e.canvas.width,e.canvas.height),e.clearColor(0,0,0,0),e.clear(e.COLOR_BUFFER_BIT),e.blendFunc(e.ONE,e.ONE_MINUS_SRC_ALPHA),e.bindBuffer(e.ARRAY_BUFFER,n.position),e.vertexAttribPointer(t.attribLocations.vertexPosition,2,e.FLOAT,!1,0,0),e.enableVertexAttribArray(t.attribLocations.vertexPosition),e.useProgram(t.program),e.uniform4f(t.uniformLocations.baseColor,r[0],r[1],r[2],r[3]),e.uniform1f(t.uniformLocations.opacityFactor,1),o?.horizontalModeEnabled?e.uniform1f(t.uniformLocations.ovalRatio,f/d):e.uniform1f(t.uniformLocations.ovalRatio,f);for(let r=0;r<7;r++){let i;i=o?.horizontalModeEnabled?-.6/2+u*2*r/6:-1+r*2/6;let s=m+r*h,c=g+r*_,k=r*Math.PI*v,A=r*Math.PI*y,j=b+r*x,M=S+r*C,N=Math.sin(O*s+k)*j+Math.sin(O*w+r*1.3)*T,P=Math.cos(O*c+A)*M+Math.cos(O*E+r*2.7)*D,F=1,I=1,L=1,R=f,z=o?.horizontalModeEnabled?p:f,U=0;if(a[r]!==void 0){let e=Math.min(a[r]/B,1);F=V+e*.7999999999999999*.2,I=V+e*.7999999999999999*1.5,L=H+e**+ee*.75,U=e*te,R=o?.horizontalModeEnabled?z/d:z}let W=o?.verticalScale??1,G=(l+P+U)*W+(1-W);e.uniform1f(t.uniformLocations.opacityFactor,L),e.uniform1f(t.uniformLocations.ovalRatio,R);let K=[F,0,0,0,0,I*W,0,0,0,0,1,0,i+N,G,0,1];e.uniformMatrix4fv(t.uniformLocations.modelViewMatrix,!1,K),e.drawArrays(e.TRIANGLES,0,n.vertexCount)}if(s){let{blurProgram:t,framebuffer1:n,texture1:r,framebuffer2:i,texture2:a,width:o,height:c,quadBuffers:l}=s,u=W;e.useProgram(t.program),e.disable(e.BLEND),e.bindBuffer(e.ARRAY_BUFFER,l.position),e.vertexAttribPointer(t.attribLocations.vertexPosition,2,e.FLOAT,!1,0,0),e.enableVertexAttribArray(t.attribLocations.vertexPosition),e.bindBuffer(e.ARRAY_BUFFER,l.texCoord),e.vertexAttribPointer(t.attribLocations.texCoord,2,e.FLOAT,!1,0,0),e.enableVertexAttribArray(t.attribLocations.texCoord),e.uniform2f(t.uniformLocations.resolution,o,c),e.uniform1f(t.uniformLocations.time,O),e.uniform1i(t.uniformLocations.texture,0);let d=r,f=i;for(let s=0;s<u.length;s++){let p=s===u.length-1;p&&(f=null),e.bindFramebuffer(e.FRAMEBUFFER,f),p?e.viewport(0,0,e.canvas.width,e.canvas.height):e.viewport(0,0,o,c),e.clear(e.COLOR_BUFFER_BIT),e.bindTexture(e.TEXTURE_2D,d),e.uniform1f(t.uniformLocations.offset,u[s]),e.uniform1f(t.uniformLocations.isLastPass,+!!p),e.drawArrays(e.TRIANGLES,0,l.vertexCount),p||(f===i?(d=a,f=n):(d=r,f=i))}e.enable(e.BLEND)}}var me="rgba(217, 119, 87, 1.0)",he=.92,ge=.9,_e=(z-85)/7,ve=r(r=>{let $=J9(25),{analyzer:i,color:a,horizontalModeEnabled:o,circleBaseY:s,verticalOffsetPx:c,bottomOverhangRatio:l}=r,u=i===void 0?null:i,d=a===void 0?me:a,f=o===void 0||o,p=c===void 0?0:c,m=l===void 0?0:l,h=J0($,0)?K0($,0,[0,0,0,0]):$[0],g=e(h),_=J0($,1)?K0($,1,[0,0,0,0]):$[1],v=e(_),y=e(.03),b,x;J1($,2,d)?(b=()=>{v.current=J(d)},x=[d],K2($,2,d,b,x)):(b=$[3],x=$[4]),n(b,x);let S=e(null),C=e(null),w=e(null),T=e(null),E=e(null),D=e(null),O=e(null),k,A;$[5]===J8?(k=()=>(O.current!==null&&(clearTimeout(O.current),O.current=null),()=>{O.current=setTimeout(()=>{S.current?.isConnected||(ue(w.current,T.current,E.current,D.current,{releaseContext:!0}),w.current=null,T.current=null,E.current=null,D.current=null)},0)}),A=[],$[5]=k,$[6]=A):(k=$[5],A=$[6]),n(k,A);let j=e(null),M=e(0),N=e(null),P=J0($,7)?K0($,7,Array.from({length:7},ye)):$[7],F=e(P),I=J0($,8)?K0($,8,Array.from({length:7},be)):$[8],L=e(I),R=J1($,9,d)?K1($,9,d,()=>{let e=S.current;if(!e)return!1;let t=window.devicePixelRatio||1,n=e.getBoundingClientRect();e.width=n.width*t,e.height=n.height*t,M.current=n.height;let{gl:r,programInfo:i,buffers:a,blurResources:o}=fe(e);return!r||!i||!a?!1:(w.current=r,T.current=i,E.current=a,D.current=o,g.current=J(d),v.current=[...g.current],!0)}):$[10],z;J5($,11,u,m,s,f,p)?(z=e=>{j.current||=e;let t=e-j.current,n=w.current,r=T.current,i=E.current;if(!g.current.every((e,t)=>Math.abs(e-v.current[t])<.001)){let e=oe(g.current,v.current,y.current);g.current=[...e]}if(u){N.current||=new Uint8Array(u.frequencyBinCount),u.getByteFrequencyData(N.current);for(let e=0;e<7;e++){let t=85+e*_e,n=t+_e,r=se(N.current,u,t,n);F.current[e]=he*F.current[e]+.07999999999999996*r,L.current[e]=ge*L.current[e]+.09999999999999998*F.current[e],isFinite(F.current[e])||(F.current[e]=0),isFinite(L.current[e])||(L.current[e]=0)}}if(n&&r&&i){let e=1/(1+m);pe(n,r,i,g.current,t,L.current,{horizontalModeEnabled:f,circleBaseY:s,yShiftClip:M.current>0?-(2*p)/(M.current*e):0,verticalScale:e},D.current)}C.current=requestAnimationFrame(z)},K5($,11,u,m,s,f,p,z)):z=$[16];let B=J2($,17,R,z)?K2($,17,R,z,e=>{if(!e){C.current&&=(cancelAnimationFrame(C.current),null);return}S.current=e,!(!w.current&&!R())&&(C.current||=requestAnimationFrame(z))}):$[19],V=`${(1+m)*100}%`,H=J1($,20,V)?K1($,20,V,{height:V}):$[21];return J2($,22,B,H)?K2($,22,B,H,t("canvas",{ref:B,className:"absolute start-0 top-0 w-full pointer-events-none",style:H,"aria-hidden":"true"})):$[24]});function ye(){return 0}function be(){return 0}var xe="rgba(106, 155, 204, 1.0)",Se=.4,Ce=r(e=>{let $=J9(13),{verticalOffsetPx:n,bottomOffsetPx:r}=e,c=n===void 0?0:n,l=r===void 0?0:r,u=s(),d=a(),f=i(),p=o(),m=f.status!=="speaking"||f.asText===!0,h=m?d??u:u,g=m?xe:void 0;if(!(!p||de()))return null;let _=+!!p,v=J1($,0,l)?K1($,0,l,l?{bottom:l}:null):$[1],y=J2($,2,_,v)?K2($,2,_,v,{width:"150%",aspectRatio:"4 / 1",contain:"layout",opacity:_,...v}):$[4],b=J4($,5,h,g,p,c)?K4($,5,h,g,p,c,p&&t(ve,{analyzer:h,color:g,horizontalModeEnabled:!0,circleBaseY:-.2,verticalOffsetPx:c,bottomOverhangRatio:Se})):$[9];return J2($,10,y,b)?K2($,10,y,b,t("div",{"data-testid":"voice-audio-visualizer",className:"absolute bottom-0 left-1/2 -translate-x-1/2 z-0 pointer-events-none transition-opacity duration-500",style:y,children:b})):$[12]});export{Ce as VoiceAudioVisualizer};