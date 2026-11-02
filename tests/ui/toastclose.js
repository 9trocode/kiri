// Every toast has a small close at its top right, the "Couldn't share" one too.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
t("a notice closes from its corner", () => {
  toast("error", "Could not do the thing");
  const el = [...document.querySelectorAll("#toasts .toast")].at(-1), x = el.querySelector(".tclose");
  if (!x) throw new Error("no close");
  const r = el.getBoundingClientRect(), b = x.getBoundingClientRect();
  if (r.right - b.right > 10 || b.top - r.top > 10) throw new Error(`close at ${b.left - r.left},${b.top - r.top}`);
  x.click();
  if (!el.classList.contains("leaving") && el.isConnected) throw new Error("still there");
});
t("Couldn't share closes without opening Share", () => {
  sh.cap = { id: 7, source: "web", limit: 2 };
  shCapToast();
  const el = document.querySelector("#toasts .toast.cap");
  if (!el?.querySelector(".tclose")) throw new Error("no close");
  const view = ui.view;
  el.querySelector(".tclose").click();
  if (el.isConnected || ui.view !== view) throw new Error("not closed, or it navigated");
});
report(R);
