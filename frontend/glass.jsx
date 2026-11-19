import React from "react";
import {createRoot} from "react-dom/client";
import LiquidGlass from "liquid-glass-react";

// React owns only the decorative material layer. Forms, focus, and application data stay
// in the existing dashboard DOM, above the refraction layer, so text never distorts.
const surfaces = new Map();
const selector = ".sidebar, .topbar, .login-inner, dialog[open]";
const contrast = matchMedia("(prefers-contrast: more)");
const transparency = matchMedia("(prefers-reduced-transparency: reduce)");
const motion = matchMedia("(prefers-reduced-motion: reduce)");
const fixedMouse = {x: 0, y: 0};

function draw(host, entry) {
  const {width, height} = host.getBoundingClientRect();
  const signature = `${Math.round(width)}:${Math.round(height)}:${contrast.matches}:${transparency.matches}:${motion.matches}`;
  if (entry.signature === signature) return;
  entry.signature = signature;
  if (!width || !height || contrast.matches || transparency.matches) {
    entry.root.render(null);
    return;
  }
  const radius = parseFloat(getComputedStyle(host).borderTopLeftRadius) || 0;
  entry.root.render(
    <LiquidGlass
      key={`${Math.round(width)}:${Math.round(height)}`}
      className="liquid-material-frame"
      mode="standard"
      displacementScale={14}
      blurAmount={0.18}
      saturation={125}
      aberrationIntensity={0.2}
      elasticity={0}
      globalMousePos={fixedMouse}
      mouseOffset={fixedMouse}
      cornerRadius={radius}
      padding="0"
      style={{position:"absolute", top:"50%", left:"50%", width:"100%", height:"100%", transition:motion.matches?"none":"all .2s ease-out"}}
    >
      <div style={{width, height}} />
    </LiquidGlass>
  );
}

function refresh() {
  for (const [host, entry] of surfaces) {
    if (!host.isConnected || !host.matches(selector) || entry.layer.parentElement !== host) {
      entry.observer.disconnect();
      entry.root.unmount();
      entry.layer.remove();
      surfaces.delete(host);
    }
  }
  for (const host of document.querySelectorAll(selector)) {
    if (!surfaces.has(host)) {
      const layer = document.createElement("div");
      layer.className = "liquid-material";
      layer.setAttribute("aria-hidden", "true");
      host.prepend(layer);
      const entry = {layer, root:createRoot(layer), observer:null};
      entry.observer = new ResizeObserver(() => draw(host, entry));
      surfaces.set(host, entry);
      entry.observer.observe(host);
    }
    draw(host, surfaces.get(host));
  }
}

let pending = false;
function schedule() {
  if (pending) return;
  pending = true;
  requestAnimationFrame(() => {pending=false; refresh();});
}
for (const media of [contrast, transparency, motion]) media.addEventListener("change", schedule);
new MutationObserver(schedule).observe(document.documentElement, {attributes:true, attributeFilter:["data-theme"]});
window.bunmailGlass = {refresh:schedule, version:"1.1.1"};
schedule();
