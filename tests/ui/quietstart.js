// The Engine card is the user's switch: a fresh Kiri reads Off with Start,
// whatever the engine does inside (setup, BuildKit, sleeping); Start makes it
// Running and it stays Running while the engine sleeps; Stop makes it Off. A
// container of yours running turns it on. Kiri's builder (its BuildKit image
// and cache volume) isn't listed in Images or Volumes, and an empty Images
// list offers no download.
kiri.on({ type: "status", version: "0.1.14", dev: false, app: "Kiri", ours: "kiri", context: "kiri" });
kiri.on({ type: "prefs", prefs: {} });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const vbtn = () => document.getElementById("vm-toggle");
const label = () => document.getElementById("vm-state").textContent;
const sbar = () => document.getElementById("statusbar").textContent;
const card = () => [label(), vbtn().hidden, vbtn().textContent, vbtn().classList.contains("primary"), document.body.dataset.vm];
const OFF = ["Off", false, "Start", true, "stopped"], ON = ["Running", false, "Stop", false, "running"];
const lists = (containers) => kiri.on({ type: "lists", containers, images: [], volumes: [], networks: [] });
const bk = { Id: "bk1", Names: ["/buildx_buildkit_kiri0"], Image: "moby/buildkit:buildx-stable-1", State: "running", Status: "Up 1 second", Labels: {} };
const web = (State) => ({ Id: "web1", Names: ["/web"], Image: "nginx", State, Status: State === "running" ? "Up 1 second" : "Exited (0)", Labels: {} });

// Fresh install: Kiri sets up inside (boots, pulls BuildKit, sleeps). The
// fixture's running containers turned the switch on; a fresh Kiri has none.
lists([]);
delete prefs.engineOn;
kiri.on({ type: "state", state: "booting" });
t("fresh Kiri setting up reads Off with Start", () => eq(card(), OFF, "booting"));
kiri.on({ type: "state", state: "running" });
lists([bk]);
t("BuildKit running inside still reads Off", () => eq(card(), OFF, "builder"));
t("status bar says off", () => eq(sbar().includes(VM + " off"), true, "status: " + sbar()));
kiri.on({ type: "state", state: "asleep" });
t("asleep without your Start reads Off", () => eq(card(), OFF, "asleep"));

t("Start sends start, remembers it, and reads Starting… then Running", () => {
  vbtn().click();
  eq([sent.at(-1)?.type, prefs.engineOn], ["start", true], "sent");
  eq(sent.some((m) => m.type === "setpref" && m.name === "engineOn" && m.value === true), true, "saved");
  kiri.on({ type: "state", state: "booting" });
  eq([label(), vbtn().textContent, vbtn().disabled], ["Starting…", "Starting…", true], "starting");
  kiri.on({ type: "state", state: "running" });
  eq(card(), ON, "running");
});
t("asleep after your Start stays Running, with no Sleeping line", () => {
  kiri.on({ type: "state", state: "asleep" });
  eq([...card(), document.getElementById("vm-sub").hidden], [...ON, true], "asleep");
});
t("waking after your Start stays Running", () => {
  kiri.on({ type: "state", state: "booting" });
  eq(card(), ON, "waking");
  kiri.on({ type: "state", state: "running" });
});
t("Settings › Resources shows the same switch", () => {
  prefs.tab = "resources"; show("settings");
  const b = document.querySelector("[data-vmpower]");
  eq([b?.textContent, b?.dataset.vmpower], ["Turn off engine", "stop"], "button");
  show("containers");
});
t("Stop sends stop and reads Off", () => {
  vbtn().click();
  eq([sent.at(-1)?.type, prefs.engineOn], ["stop", false], "sent");
  kiri.on({ type: "state", state: "stopping" });
  eq(label(), "Stopping…", "stopping");
  kiri.on({ type: "state", state: "stopped" });
  eq(card(), OFF, "stopped");
});

// A container you start yourself (docker run) turns it on.
t("a container of yours running reads Running", () => {
  kiri.on({ type: "state", state: "booting" });
  eq(card(), OFF, "booting by itself");
  kiri.on({ type: "state", state: "running" });
  lists([web("running")]);
  eq([card(), prefs.engineOn], [ON, true], "yours");
  lists([web("exited")]);
  eq(card(), ON, "still on after it exits");
});
t("Settings › Resources Start engine starts it", () => {
  kiri.on({ type: "state", state: "stopped" });
  prefs.tab = "resources"; show("settings");
  const b = document.querySelector("[data-vmpower]");
  eq([b?.textContent, b?.dataset.vmpower], ["Start engine", "start"], "button");
  b.click();
  eq([sent.at(-1)?.type, prefs.engineOn], ["start", true], "sent");
  show("containers");
  kiri.on({ type: "state", state: "running" });
});
t("a Kiri from before the switch with containers reads Running", () => {
  delete prefs.engineOn;
  lists([web("exited")]);
  eq(card(), ON, "upgrade");
});

const img = (id, tags, size = 1e8) => ({ Id: `sha256:${id}`, RepoTags: tags, RepoDigests: [], Created: 1, Size: size, Containers: 0 });
kiri.on({ type: "lists", containers: [], networks: [],
  images: [img("bk", ["moby/buildkit:buildx-stable-1"], 112e6), img("ng", ["nginx:latest"]), img("bk2", ["moby/buildkit:v0.24.0"])],
  volumes: [{ Name: "buildx_buildkit_kiri-build0_state", Driver: "local" }, { Name: "pgdata", Driver: "local" }] });
t("Kiri's builder image isn't in Images; nginx and another BuildKit tag are", () => {
  eq(ui.images.map((i) => i.RepoTags[0]), ["nginx:latest", "moby/buildkit:v0.24.0"], "images");
});
t("its cache volume isn't in Volumes", () => eq(ui.volumes.map((v) => v.Name), ["pgdata"], "volumes"));
t("Settings → Builds shows the builder and Clear build cache", () => {
  const html = builderPref();
  if (!/Kiri's builder/.test(html) || !/data-buildprune/.test(html)) throw new Error(html);
});

kiri.on({ type: "lists", containers: [], networks: [], images: [img("bk", ["moby/buildkit:buildx-stable-1"])], volumes: [] });
show("images");
t("Images counts nothing and offers no download when only the builder is there", () => {
  eq([ui.images.length, !!document.getElementById("empty-pull"), /docker pull alpine/.test(document.getElementById("content").textContent)], [0, false, true], "empty");
});
kiri.on({ type: "lists", containers: [], networks: [], images: [], volumes: [] });
t("before any build, Settings → Builds says the builder isn't made yet", () => {
  if (!/Not set up yet/.test(builderPref())) throw new Error(builderPref());
});
report(R);
