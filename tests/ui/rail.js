// The sidebar folds to a rail of icons and back, by its button or ⌘B /
// Ctrl+B, and stays as it was left. Every view still opens from the rail.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const railOn = () => document.body.classList.contains("rail");
t("the button folds it, and the icons say their names", () => {
  setRail(false);
  $("rail-toggle").click();
  if (!railOn()) throw new Error("not folded");
  const b = document.querySelector('nav button[data-view="images"]');
  if (b.title !== "Images" || getComputedStyle(b.querySelector(".lbl")).display !== "none") throw new Error("label shown or no title");
  if (localStorage.getItem("kiri.rail") !== "1") throw new Error("not remembered");
  if (!/Show the sidebar/.test($("rail-toggle").title) || $("rail-toggle").getAttribute("aria-pressed") !== "false") throw new Error($("rail-toggle").title);
});
t("views still open from the rail", () => {
  document.querySelector('nav button[data-view="volumes"]').click();
  if (ui.view !== "volumes") throw new Error(ui.view);
  show("containers");
});
t("the shortcut opens it again", () => {
  document.dispatchEvent(new KeyboardEvent("keydown", { key: "b", metaKey: !WIN, ctrlKey: WIN, bubbles: true }));
  if (railOn()) throw new Error("still folded");
  if (document.querySelector('nav button[data-view="images"]').hasAttribute("title")) throw new Error("title left");
});
t("the shortcut leaves typing alone", () => {
  const i = document.createElement("input"); document.body.append(i); i.focus();
  i.dispatchEvent(new KeyboardEvent("keydown", { key: "b", metaKey: !WIN, ctrlKey: WIN, bubbles: true }));
  i.remove();
  if (railOn()) throw new Error("folded while typing");
  try { localStorage.removeItem("kiri.rail"); } catch {}
});
report(R);
