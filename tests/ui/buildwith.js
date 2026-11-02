// Which builder docker build uses: the first-run question (after the welcome,
// Escape keeps Docker's) and Settings → Builds, which switches at once.
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const last = (cmd) => sent.filter((m) => m.type === cmd).at(-1);
const dlg = () => document.getElementById("bw-dlg");

t("asked after the welcome, not over it", () => {
  kiri.on({ type: "welcome" });
  kiri.on({ type: "buildask", engine: "OrbStack" });
  if (dlg()) throw new Error("the build question opened over the welcome");
  document.getElementById("welcome-go").click();
  if (!dlg()) throw new Error("no build question after Continue");
  const text = dlg().textContent;
  if (!/You already have OrbStack/.test(text) || !/Keep building with Docker/.test(text) || !/Build with Kiri/.test(text)) throw new Error("it says: " + text.slice(0, 120));
  if (document.activeElement?.dataset.bwpick !== "docker") throw new Error("Keep building with Docker isn't the default");
});
t("Escape keeps Docker's builder", () => {
  document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
  if (dlg()) throw new Error("still open");
  if (last("buildwith")?.value !== "docker") throw new Error("sent " + JSON.stringify(last("buildwith")));
});
t("Build with Kiri sends kiri", () => {
  kiri.on({ type: "buildask", engine: "Docker Desktop" });
  dlg().querySelector('[data-bwpick="kiri"]').click();
  if (dlg()) throw new Error("still open");
  if (last("buildwith")?.value !== "kiri") throw new Error("sent " + JSON.stringify(last("buildwith")));
});
t("Settings → Builds asks for the state and shows it", () => {
  show("settings");
  document.querySelector('[data-stab="builds"]').click();
  if (!last("buildwith") || last("buildwith").value !== undefined) throw new Error("didn't ask for the state");
  kiri.on({ type: "buildwith", with: "docker", ask: false, engine: "OrbStack", taken: false, ready: true, builder: "kiri-dev-build", context: "kiri-dev" });
  const pane = document.querySelector(".settings .pane");
  if (!/OrbStack's builds stay in OrbStack/.test(pane.textContent)) throw new Error("pane says: " + pane.textContent.trim().slice(0, 120));
  if (pane.querySelector('[data-bw="docker"]').getAttribute("aria-pressed") !== "true") throw new Error("Docker isn't pressed");
});
t("switching to Kiri sends it and shows it at once", () => {
  const n = sent.length;
  document.querySelector('[data-bw="kiri"]').click();
  if (sent.length !== n + 1 || last("buildwith").value !== "kiri") throw new Error("sent " + JSON.stringify(sent.slice(n)));
  if (document.querySelector('[data-bw="kiri"]').getAttribute("aria-pressed") !== "true") throw new Error("Kiri isn't pressed");
  kiri.on({ type: "buildwith", with: "kiri", ask: false, engine: "OrbStack", taken: true, ready: true });
  if (!/OrbStack doesn't see them/.test(document.querySelector(".settings .pane").textContent)) throw new Error("no word about OrbStack");
});
t("pressing the chosen one again does nothing", () => {
  const n = sent.length;
  document.querySelector('[data-bw="kiri"]').click();
  if (sent.length !== n) throw new Error("sent " + JSON.stringify(sent.slice(n)));
});
t("Kiri chosen but another builder picked since: Kiri again takes it back", () => {
  kiri.on({ type: "buildwith", with: "kiri", ask: false, engine: null, taken: false, ready: true });
  if (!/Choose Kiri again/.test(document.querySelector(".settings .pane").textContent)) throw new Error("no note");
  document.querySelector('[data-bw="kiri"]').click();
  if (last("buildwith").value !== "kiri") throw new Error("didn't send");
});
t("back to Docker", () => {
  document.querySelector('[data-bw="docker"]').click();
  if (last("buildwith").value !== "docker") throw new Error("didn't send");
});
report(R);
