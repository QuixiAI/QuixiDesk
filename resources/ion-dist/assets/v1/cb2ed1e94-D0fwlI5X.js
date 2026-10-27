import{Ba as e,Co as t,Ha as n,ho as r}from"./vendor-frame-CnSY1sxv.js";import{$f as J2,Qf as J1,Zf as J0,ap as K0,ep as J3,ip as J9,lp as K4,op as K1,sp as K2}from"./shared-frame-6jtSkIit.js";import{cp as i}from"./shared-25-DOWZRSlq.js";import{n as a,r as o,t as s}from"./c67494563-CxlQ3i1n.js";var c=`#version 300 es
in vec2 a_position;
out vec2 v_uv;
uniform vec2 u_resolution;
void main() {
  // pixel-space UV so fragment can do floor(uv / gap) without aspect distortion
  v_uv = (a_position * 0.5 + 0.5) * u_resolution;
  gl_Position = vec4(a_position, 0.0, 1.0);
}
`,l=`#version 300 es
precision highp float;
uniform vec2 u_resolution;
uniform float u_time;
uniform vec4 u_color;
in vec2 v_uv;
out vec4 fragColor;

const float GAP   = 22.0;   // px between cell centers
const float DOT_R = 1.6;    // peak radius px \u2014 large enough to read round
const float R_MIN = 1.0;    // trough radius px \u2014 still a circle, not a smear

const float ARMS  = 2.0;    // spiral arm count
const float PITCH = 0.08;   // radial phase rate \u2014 lower = looser spiral
const float SPIN  = 0.25;   // arm rotation rad/sec
const float DRIFT = 0.035;  // spiral origin Lissajous speed

// Arm half-width in dArm units (0..1). Beyond this: alpha = 0, dots invisible.
// Gamma 2.2 roughly matches sRGB \u2014 perceptually the brightness steps look
// more even than a linear ramp would at these low alphas.
const float WIDTH = 0.55;
const float GAMMA = 2.2;

// Sheet ripple \u2014 dots are displaced off their lattice points by a pair of
// crossed traveling waves. Runs independent of (and slower than) the spiral.
const float RIPPLE_AMP  = 3.0;    // peak displacement px
const float RIPPLE_FREQ = 0.020;  // wave number (cycles per cell)
const float RIPPLE_VEL  = 0.12;   // phase velocity rad/sec
const float RIPPLE_TURN = 0.015;  // wave-heading drift rad/sec

void main() {
  vec2 cell = floor(v_uv / GAP);

  // Two waves at heading \u03B8 and \u03B8+~60\u00B0; heading drifts so the interference
  // pattern slowly reorients. Displacement is along each wave's travel axis
  // \u2014 a plane wave pushes points in-line, not sideways.
  float h = u_time * RIPPLE_TURN;
  vec2 k1 = vec2(cos(h), sin(h));
  vec2 k2 = vec2(cos(h + 1.05), sin(h + 1.05));
  float s1 = sin(dot(cell, k1) * RIPPLE_FREQ * 6.2831853 - u_time * RIPPLE_VEL);
  float s2 = sin(dot(cell, k2) * RIPPLE_FREQ * 6.2831853 - u_time * RIPPLE_VEL * 0.8);
  vec2 disp = (k1 * s1 + k2 * s2) * RIPPLE_AMP * 0.5;

  vec2 local = v_uv - (cell + 0.5) * GAP - disp;
  float dist = length(local);

  // Spiral origin wanders on a 3:2 Lissajous in cell space.
  vec2 gridSize = u_resolution / GAP;
  vec2 origin = gridSize * 0.5 + gridSize * 0.35 * vec2(
    sin(u_time * DRIFT * 3.0),
    cos(u_time * DRIFT * 2.0)
  );

  vec2 d = cell - origin;
  float rad = length(d);
  float ang = atan(d.y, d.x);

  // Linear distance from nearest arm spine, 0 = on spine, 1 = mid-trough.
  float phase = ang * ARMS + rad * PITCH - u_time * SPIN;
  float dArm = acos(cos(phase)) / 3.14159265;

  // Normalized position within the arm, 1 at spine \u2192 0 at WIDTH, clamped.
  float t = max(1.0 - dArm / WIDTH, 0.0);

  float a = pow(t, GAMMA);
  float r = mix(R_MIN, DOT_R, t);

  float aa = fwidth(dist);
  float mask = 1.0 - smoothstep(r - aa, r + aa, dist);

  // u_color is premultiplied at upload; with ONE, ONE_MINUS_SRC_ALPHA the
  // per-fragment modulation must scale all four channels or rgb > alpha
  // violates the invariant (additive halos on dark-mode dot edges).
  fragColor = u_color * (a * mask);
}
`;function u(e,t,n){let r=e.createShader(t);return r?(e.shaderSource(r,n),e.compileShader(r),e.getShaderParameter(r,e.COMPILE_STATUS)?r:(console.warn("[epitaxy-shader] compile:",e.getShaderInfoLog(r)),e.deleteShader(r),null)):null}function d(e,t){e.useProgram(t)}function f(e,t,n){let r=u(e,e.VERTEX_SHADER,t),i=u(e,e.FRAGMENT_SHADER,n);if(!r||!i)return null;let a=e.createProgram();return a?(e.attachShader(a,r),e.attachShader(a,i),e.linkProgram(a),e.deleteShader(r),e.deleteShader(i),e.getProgramParameter(a,e.LINK_STATUS)?a:(console.warn("[epitaxy-shader] link:",e.getProgramInfoLog(a)),e.deleteProgram(a),null)):null}function p(e,t,n){if(t===0)return[n,n,n];let r=(e,t,n)=>(n<0&&(n+=1),n>1&&--n,n<1/6?e+(t-e)*6*n:n<1/2?t:n<2/3?e+(t-e)*(2/3-n)*6:e),i=n<.5?n*(1+t):n+t-n*t,a=2*n-i;return[r(a,i,e+1/3),r(a,i,e),r(a,i,e-1/3)]}function m(e){let t=e.trim(),n=t.match(/^hsla?\(\s*([\d.]+)\s*,\s*([\d.]+)%\s*,\s*([\d.]+)%\s*(?:,\s*([\d.]+))?\s*\)$/i);if(n){let e=parseFloat(n[1])/360,t=parseFloat(n[2])/100,r=parseFloat(n[3])/100,i=n[4]===void 0?1:parseFloat(n[4]),[a,o,s]=p(e,t,r);return[a,o,s,i]}let r=t.match(/^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+))?\s*\)$/i);if(r)return[parseFloat(r[1])/255,parseFloat(r[2])/255,parseFloat(r[3])/255,r[4]===void 0?1:parseFloat(r[4])];let i=t.match(/^#([0-9a-f]{6})([0-9a-f]{2})?$/i);if(i){let e=parseInt(i[1],16),t=i[2]===void 0?1:parseInt(i[2],16)/255;return[(e>>16&255)/255,(e>>8&255)/255,(e&255)/255,t]}return[.5,.5,.5,.5]}var h=4147200,g=new Float32Array([-1,-1,1,-1,-1,1,-1,1,1,-1,1,1]);function _(e){let $=J9(5),{fragmentSource:n,color:o,onSetup:l}=e,u=t(null),p,_;return J3($,0,o,n,l)?(p=()=>{let e=u.current;if(!e)return;let t=e.getContext("webgl2",{alpha:!0,antialias:!1,depth:!1,stencil:!1,premultipliedAlpha:!0});if(!t)return;let r=f(t,c,n);if(!r)return;d(t,r);let p=l?.(t,r),_=t.createBuffer();t.bindBuffer(t.ARRAY_BUFFER,_),t.bufferData(t.ARRAY_BUFFER,g,t.STATIC_DRAW);let v=t.getAttribLocation(r,"a_position");t.enableVertexAttribArray(v),t.vertexAttribPointer(v,2,t.FLOAT,!1,0,0);let y=t.getUniformLocation(r,"u_time"),b=t.getUniformLocation(r,"u_resolution"),x=t.getUniformLocation(r,"u_color");t.enable(t.BLEND),t.blendFunc(t.ONE,t.ONE_MINUS_SRC_ALPHA),t.clearColor(0,0,0,0);let S=!0,C=(t,n,r)=>{let i=Math.max(1,Math.round(t*r)),a=Math.max(1,Math.round(n*r)),o=i*a;if(o>h){let e=Math.sqrt(h/o);i=Math.max(1,Math.round(i*e)),a=Math.max(1,Math.round(a*e))}(e.width!==i||e.height!==a)&&(e.width=i,e.height=a,S=!0)},w=new(i(e))(t=>{let n=t[0],r=n.devicePixelContentBoxSize?.[0];if(r)C(r.inlineSize,r.blockSize,1);else{let t=n.borderBoxSize?.[0]??n.contentBoxSize?.[0],r=t?.inlineSize??e.clientWidth,i=t?.blockSize??e.clientHeight;C(r,i,window.devicePixelRatio||1)}A()});w.observe(e),(()=>{let[e,n,r,i]=m(o??"hsla(0, 0%, 50%, 0.5)");t.uniform4f(x,e*i,n*i,r*i,i)})();let T=+!!s(),E=0,D=Math.random()*1e6,O=performance.now(),k=!1,A=()=>{S&&=(t.viewport(0,0,e.width,e.height),t.uniform2f(b,e.width,e.height),!1),t.uniform1f(y,D*.001),t.clear(t.COLOR_BUFFER_BIT),t.drawArrays(t.TRIANGLES,0,6)},j=e=>{D+=(e-O)*T,O=e,A(),E=requestAnimationFrame(j)},M=()=>{k||T===0||(k=!0,O=performance.now(),E=requestAnimationFrame(j))},N=()=>{k&&(k=!1,cancelAnimationFrame(E))},P=a(e=>{T=+!!e,T===0?(N(),A()):document.hidden||M()}),F=()=>{document.hidden?N():M()};return document.addEventListener("visibilitychange",F),C(e.clientWidth||1,e.clientHeight||1,window.devicePixelRatio||1),A(),document.hidden||M(),()=>{N(),p?.(),w.disconnect(),P(),document.removeEventListener("visibilitychange",F),t.deleteProgram(r),t.deleteBuffer(_)}},_=[n,o,l],K4($,0,o,n,l,p,_)):(p=$[3],_=$[4]),r(p,_),u}function v(t){let $=J9(6),r=_(t),{shouldAnimate:i}=o(),a=i&&"animate-[fade_0.6s_ease-out]",s=J1($,0,a)?K1($,0,a,e("absolute inset-0 w-full h-full pointer-events-none",a)):$[1],c=J0($,2)?K0($,2,{zIndex:-1}):$[2];return J2($,3,r,s)?K2($,3,r,s,n("canvas",{ref:r,className:s,style:c,"aria-hidden":!0})):$[5]}function y(){let $=J9(1);return J0($,0)?K0($,0,n(v,{fragmentSource:l})):$[0]}export{y as DotGridShader};