// What Kiri's window costs, beside its version in the status bar.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
t("the window asks what it costs, and shows it beside the version", () => {
  kiri.on({ type: "usage", mem: 382 * 1e6, cpu: 1.24, parts: [{ name: "Window", mem: 35 * 1e6, cpu: 0.2, n: 1 }, { name: "Graphics", mem: 250 * 1e6, cpu: 0.9, n: 1 }, { name: "Page", mem: 97 * 1e6, cpu: 0.14, n: 1 }] });
  const el = document.querySelector(".statusbar .use");
  if (!/382(\.0)? MB · 1\.2% CPU/.test(el?.textContent || "")) throw new Error(el?.textContent);
  if (!/Graphics: 250(\.0)? MB/.test(el.title) || !/not counting the Linux VM/.test(el.title)) throw new Error(el.title);
});
report(R);
