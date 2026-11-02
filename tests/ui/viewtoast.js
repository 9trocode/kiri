// The "Started …" toast has View, which opens that container's page, even
// when the list doesn't have it yet.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
show("images");
kiri.on({ type: "notice", kind: "ok", text: "Started brave_hopper on 8081", open: "n1" });
const view = () => [...document.querySelectorAll("#toasts [data-tview]")].at(-1);
t("the toast has View", () => { if (!view()) throw new Error("no View"); });
view().click();
t("View waits for the list, then opens the container's page", () => {
  if (ui.view !== "containers" || ui.detail) throw new Error("opened too early");
  kiri.on({ type: "lists", containers: [...ui.containers, { Id: "n1", Names: ["/brave_hopper"], Image: "nginx:alpine", State: "running", Status: "Up 1 second", Created: Date.now() / 1000, Ports: [], Labels: {} }], images: [], volumes: [], networks: [] });
  if (ui.detail?.id !== "n1") throw new Error("detail: " + JSON.stringify(ui.detail?.id));
});
t("an error toast has no View", () => {
  kiri.on({ type: "notice", kind: "error", text: "No such image", open: "n1" });
  if (document.querySelector("#toasts .toast.error [data-tview]")) throw new Error("View on an error");
});
report(R);
