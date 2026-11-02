// Run only says a container started once it stays up: a container that stops
// with an error in its first seconds is said to have failed, with why; and
// Run on a Kubernetes image points to Kiri's own cluster instead of starting
// a copy. kiri-k8s is marked Kubernetes, and stopping or removing it says it
// turns Kubernetes off.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const nowS = Math.floor(Date.now() / 1000);
const img = (tag, i) => ({ Id: "sha256:" + String(i).repeat(64), RepoTags: [tag], Size: 1e8, Created: nowS - 3600 });
const IMGS = [img("kiri-test/crash:1", 1), img("kiri-test/fine:1", 2), img("rancher/k3s:v1.31.4-k3s1", 3)];
const C = (id, image, state, extra = {}) => ({ Id: id, Names: ["/" + id], Image: image, State: state, Status: state, Created: nowS, Ports: [], Labels: {}, ...extra });
const lists = (cs) => kiri.on({ type: "lists", containers: cs, images: IMGS, volumes: [], networks: [] });
lists([]);
show("images");
const crash = "kiri-test/crash:1", fine = "kiri-test/fine:1";
t("right after Run, the button still spins and nothing says started", () => {
  kiri.on({ type: "notice", kind: "ok", text: "Started c1", open: "c1", image: crash });
  kiri.on({ type: "ran", name: crash });
  if (!/rolling/.test(runBtn(crash))) throw new Error(runBtn(crash));
  if (document.querySelector(".run-tip") || document.querySelector("#toasts .toast")) throw new Error("said too soon");
});
t("it stops with an error: a red tip says it stopped, and asks its logs why", () => {
  lists([C("c1", crash, "exited", { KiriExitCode: 1, KiriStartedAt: nowS - 2, KiriFinishedAt: nowS })]);
  const tip = document.querySelector(".run-tip.fail");
  if (!tip || !/c1 stopped after 2 s/.test(tip.textContent)) throw new Error(tip?.outerHTML || "no red tip");
  if (!sent.some((m) => m.type === "lastlogs" && m.id === "c1")) throw new Error("didn't ask the logs");
  if (/rolling/.test(runBtn(crash))) throw new Error("still spinning");
  if (ui.runWatch.has("c1")) throw new Error("still watched");
});
t("the log lines turn it into the reason", () => {
  kiri.on({ type: "lastlogs", id: "c1", lines: ["time=\"x\" level=fatal msg=\"Error: failed to evacuate root cgroup: mkdir /sys/fs/cgroup/init: read-only file system\""] });
  const tip = document.querySelector(".run-tip.fail");
  if (!tip || !/stopped/.test(tip.textContent) || !tip.querySelector("[data-tlogs]")) throw new Error(tip?.outerHTML || "no tip");
  runSay("c1");
  if (document.querySelector("#toasts .toast")) throw new Error("a started toast after it failed");
});
t("one that stays up is said started a few seconds later", () => {
  document.querySelector(".run-tip")?.remove();
  kiri.on({ type: "notice", kind: "ok", text: "Started c2", open: "c2", image: fine });
  lists([C("c2", fine, "running")]);
  if (document.querySelector(".run-tip")) throw new Error("too soon");
  runSay("c2");
  const tip = document.querySelector(".run-tip:not(.fail)");
  if (!tip || !/Started c2/.test(tip.textContent)) throw new Error(tip?.outerHTML || "no tip");
});
t("off Images, a failure is an error toast with See why", () => {
  document.querySelector(".run-tip")?.remove();
  kiri.on({ type: "notice", kind: "ok", text: "Started c3", open: "c3", image: crash });
  show("volumes");
  lists([C("c3", crash, "exited", { KiriExitCode: 1, KiriStartedAt: nowS - 1, KiriFinishedAt: nowS })]);
  const el = [...document.querySelectorAll("#toasts .toast")].find((x) => /c3 stopped/.test(x.textContent));
  if (!el || !/See why/.test(el.textContent)) throw new Error(el?.outerHTML || "no toast");
  el.remove();
});
t("a clean exit is a finish, not a failure", () => {
  show("volumes");
  kiri.on({ type: "notice", kind: "ok", text: "Started c4", open: "c4", image: fine });
  lists([C("c4", fine, "exited", { KiriExitCode: 0, KiriStartedAt: nowS - 1, KiriFinishedAt: nowS })]);
  const el = [...document.querySelectorAll("#toasts .toast")].find((x) => /c4/.test(x.textContent));
  if (!el || !/ran and finished/.test(el.textContent) || el.classList.contains("error")) throw new Error(el?.outerHTML || "no toast");
  el.remove();
});
const dialog = () => document.querySelector(".scrim .dialog");
t("Run on a Kubernetes image doesn't start a copy", () => {
  for (const n of ["rancher/k3s:v1.31.4-k3s1", "docker.io/rancher/k3s:latest", "kindest/node:v1.30.0", "k3s"]) if (!isK8sImage(n)) throw new Error(n);
  for (const n of ["nginx:alpine", "rancher/rancher:latest", "bitnami/kubectl"]) if (isK8sImage(n)) throw new Error(n);
  sent.length = 0;
  runImage("rancher/k3s:v1.31.4-k3s1");
  if (sent.some((m) => m.type === "run")) throw new Error("sent run");
  if (!/This is Kubernetes. Kiri runs it for you/.test(dialog()?.textContent)) throw new Error(dialog()?.textContent);
});
t("Kubernetes off: Turn on Kubernetes opens its settings", () => {
  const go = dialog().querySelector("[data-go]");
  if (go.textContent !== "Turn on Kubernetes" || go.classList.contains("danger")) throw new Error(go.outerHTML);
  go.click();
  if (ui.view !== "settings" || prefs.tab !== "kubernetes") throw new Error(ui.view + " " + prefs.tab);
});
const kk = C("k8sid", "rancher/k3s:v1.31.4-k3s1", "running", { Names: ["/kiri-k8s"] });
t("Kubernetes on: Open Pods", () => {
  lists([kk]);
  runImage("rancher/k3s:v1.31.4-k3s1");
  const go = dialog().querySelector("[data-go]");
  if (go.textContent !== "Open Pods") throw new Error(go.textContent);
  go.click();
  if (ui.view !== "pods") throw new Error(ui.view);
});
t("kiri-k8s is marked Kubernetes in Containers", () => {
  show("containers");
  const row = document.querySelector('#content tr[data-key="k8sid"]');
  // The Kubernetes logo beside its name, named for screen readers.
  const mark = row?.querySelector(".k8s-mark");
  if (mark?.getAttribute("aria-label") !== "Kubernetes" || !mark.querySelector("svg path")) throw new Error(row ? "no Kubernetes mark" : "no row");
  const other = C("plain", "nginx:alpine", "running");
  lists([kk, other]);
  if (document.querySelector('#content tr[data-key="plain"] .k8s-mark')) throw new Error("another container is marked too");
  lists([kk]);
});
t("stopping kiri-k8s says it turns Kubernetes off, then stops it", () => {
  sent.length = 0;
  containerAction("stop", "k8sid");
  if (sent.some((m) => m.type === "container") || !/turns Kubernetes off/.test(dialog()?.textContent)) throw new Error(dialog()?.textContent);
  dialog().querySelector("[data-go]").click();
  if (!sent.some((m) => m.type === "container" && m.action === "stop" && m.id === "k8sid")) throw new Error(JSON.stringify(sent));
});
t("removing kiri-k8s says what stays", () => {
  containerAction("remove", "k8sid");
  if (!/turns Kubernetes off/.test(dialog()?.textContent) || !/stays in the kiri-k8s volume/.test(dialog()?.textContent)) throw new Error(dialog()?.textContent);
  dialog().querySelector("[data-close]").click();
});
report(R);
