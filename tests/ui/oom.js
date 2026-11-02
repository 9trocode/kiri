// Out of memory: the flat red notice, and each place that says memory is why.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const text = (el) => el?.textContent.replace(/\s+/g, " ").trim() || "";
const vm = (used, stall, kills) => kiri.on({ type: "vm", awake: true, used_mib: used, target_mib: 1024, ceiling_mib: 1024, running: 3, cpus: 1, cpu_pct: 4, stall_pct: stall, oom_kills: kills });
const list = (extra) => kiri.on({ type: "lists", containers: [...ui.containers.filter((c) => c.Id !== "g1"), ...extra], images: ui.images, volumes: ui.volumes, networks: ui.networks });
const STOPPED = Math.floor(Date.now() / 1000) - 30;
const grafana = (oom) => ({ Id: "g1", Names: ["/grafana"], Image: "grafana/grafana", State: oom === null ? "running" : "exited", Status: "", KiriExitCode: 137, KiriOOMKilled: !!oom, KiriFinishedAt: STOPPED, Ports: [], Labels: {} });
show("containers");
t("plenty of memory: the status bar is grey and nothing is said", () => {
  vm(400, 0, 0);
  eq([!!document.querySelector("#statusbar .memfull"), document.querySelectorAll(".oom").length], [false, 0], "quiet");
});
t("stalled on memory: the status bar says Memory full, in red", () => {
  vm(990, 91.73, 0);
  eq(text(document.querySelector("#statusbar .memfull")), "Memory full · 990 MB of 1 GB", "status bar");
});
t("a container stopped for memory reads Out of memory, and a notice says so once", () => {
  list([grafana(null)]);
  list([grafana(true)]);
  const row = [...document.querySelectorAll("tr.row")].find((r) => r.textContent.includes("grafana"));
  eq(text(row.querySelector(".pill")), "Out of memory", "state");
  eq(row.querySelector(".pill").classList.contains("failed"), true, "red");
  const n = [...document.querySelectorAll(".oom.toast")];
  eq(n.length, 1, "one notice");
  eq(text(n[0].querySelector("b")), "grafana ran out of memory", "notice");
  list([grafana(true)]);
  eq(document.querySelectorAll(".oom.toast").length, 1, "not again for the same stop");
  n[0].querySelector("[data-oomx]").click();
});
t("its page says when, with Start again and More memory", () => {
  openDetail("g1");
  const line = document.querySelector("#content .oom");
  eq(text(line.querySelector("b")), "Stopped: out of memory", "line");
  eq([...line.querySelectorAll("button")].map(text), ["Start again", "More memory"], "buttons");
  closeDetail(); show("containers");
});
t("a waiting screen says memory is why", () => {
  eq(text(document.createRange().createContextualFragment(`<div>${oomWait()}</div>`).firstChild.querySelector("b")), "Slow because the VM is out of memory", "wait line");
  vm(400, 0, 1);
  eq(oomWait(), "", "quiet with memory to spare");
});
t("a pod Kubernetes OOMKilled reads Out of memory", () => {
  const html = podPill({ status: "OOMKilled", allReady: false });
  eq([/Out of memory/.test(html), /failed/.test(html)], [true, true], "pill");
});
t("More memory opens Settings → Resources", () => {
  vm(990, 91.73, 1);
  document.querySelector("#statusbar .memfull").click();
  eq([ui.view, prefs.tab], ["settings", "resources"], "where");
});
// Last: a kill inside a container (no container stopped) still gets the
// notice, a moment later, once the lists have had their say.
document.querySelectorAll(".oom.toast").forEach((x) => x.remove());
vm(990, 91.73, 2);
setTimeout(() => {
  t("a kill inside a container, with no container stopped, still gets the notice", () =>
    eq(text(document.querySelector(".oom.toast b")), "Linux stopped a process for memory", "notice"));
  report(R);
}, 1700);
