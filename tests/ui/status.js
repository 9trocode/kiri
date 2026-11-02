// The status bar along the bottom of the window.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
t("Kubernetes shows its logo before its version", () => {
  ui.vm = "running";
  ui.containers.push({ Id: "k8s1", Names: ["/kiri-k8s"], Image: "rancher/k3s:v1.37.1-k3s1", State: "running", Ports: [] });
  renderStatus();
  const item = [...document.querySelectorAll("#statusbar .item")].find((x) => x.textContent.includes("Kubernetes"));
  eq(item?.textContent.trim(), "Kubernetes v1.37.1", "text");
  const r = item.querySelector("svg.k8s").getBoundingClientRect();
  eq([r.width, r.height], [13, 13], "logo size");
  ui.containers.pop(); renderStatus();
});
report(R);
