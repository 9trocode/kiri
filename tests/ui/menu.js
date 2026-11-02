// Right-click: no browser menu (Reload…) except in text fields and on selected text.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const menu = (el) => { const ev = new MouseEvent("contextmenu", { bubbles: true, cancelable: true }); el.dispatchEvent(ev); return !ev.defaultPrevented; };
t("no browser menu on the Images view", () => { getSelection().removeAllRanges(); show("images"); eq(menu($("content")), false, "menu shown"); });
t("a text field keeps its menu", () => { const i = document.createElement("input"); document.body.append(i); eq(menu(i), true, "menu shown"); i.remove(); });
t("selected text keeps its menu", () => {
  const p = document.createElement("p"); p.textContent = "copy me"; p.style.userSelect = p.style.webkitUserSelect = "text"; document.body.append(p);
  getSelection().selectAllChildren(p); eq(menu(p), true, "menu shown"); getSelection().removeAllRanges(); p.remove();
});
report(R);
