import { createRoot } from "react-dom/client";
import { App } from "./App";
import { initStorage } from "./core/storage";
import "./styles.css";

// iPad Safari has no right-click: a finger long-press opens the same menus.
(function longPress() {
  let timer = 0, start: [number, number] | null = null, fired = false;
  const cancel = () => { clearTimeout(timer); start = null; };
  addEventListener("pointerdown", (e) => {
    fired = false;
    if (e.pointerType !== "touch") return;
    const target = e.target as Element;
    if (target.closest(".board")) return; // the writing surface handles its own touches
    start = [e.clientX, e.clientY];
    timer = window.setTimeout(() => {
      if (!start) return;
      fired = true;
      target.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: start[0], clientY: start[1] }));
      navigator.vibrate?.(10);
    }, 500);
  }, true);
  addEventListener("pointermove", (e) => { if (start && Math.hypot(e.clientX - start[0], e.clientY - start[1]) > 10) cancel(); }, true);
  addEventListener("pointerup", cancel, true);
  addEventListener("pointercancel", cancel, true);
  // The tap that ends a long-press must not also trigger the item.
  addEventListener("click", (e) => { if (fired) { e.stopPropagation(); e.preventDefault(); fired = false; } }, true);
})();

const root = createRoot(document.getElementById("root")!);
root.render(<div className="loading">Loading Notebook…</div>);
initStorage().then(() => root.render(<App />));
