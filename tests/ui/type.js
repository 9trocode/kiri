// Type: every text size in the window comes from the scale on :root, so zoom
// moves them together and no screen drifts. Glyphs drawn with text (arrows, ×)
// are icons and keep their own size.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const GLYPHS = [".ctx-flow .arr", ".sh-chev", ".sc-close", ".sh-x", "tr.group .chev"];
t("every font size is on the scale", () => {
  const off = [];
  for (const sheet of document.styleSheets) for (const r of sheet.cssRules) {
    if (!r.style || GLYPHS.some((g) => r.selectorText?.startsWith(g))) continue;
    const fs = r.style.getPropertyValue("font-size");
    if (fs && /\dpx/.test(fs) && r.selectorText !== ":root") off.push(`${r.selectorText}: ${fs}`);
  }
  eq(off, [], "sizes outside the scale");
});
t("the sizes", () => {
  const v = (n) => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
  eq(["--fs-small", "--fs", "--fs-mono", "--fs-head", "--fs-title"].map(v), ["11px", "12px", "11.5px", "14px", "18px"], "scale");
});
t("the sidebar is the size everything else is", () => {
  const px = (sel) => getComputedStyle(document.querySelector(sel)).fontSize;
  eq([px("nav button"), px("body")], ["12px", "12px"], "sidebar and body");
});
t("values in code sit half a step under the text", () => {
  const m = document.createElement("span"); m.className = "mono"; document.body.append(m);
  const px = getComputedStyle(m).fontSize; m.remove(); eq(px, "11.5px", "mono");
});
report(R);
