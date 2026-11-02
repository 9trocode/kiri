// Log text: small unless you choose a size in Settings → Appearance.
kiri.on({ type: "status", version: "0.1.4", dev: true, app: "Kiri Dev", ours: "kiri-dev", context: "kiri-dev" });
const R = [];
const t = (name, fn) => { try { fn(); R.push("PASS " + name); } catch (e) { R.push("FAIL " + name + ": " + e.message); } };
const eq = (a, b, m) => { if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(m + ": got " + JSON.stringify(a) + " want " + JSON.stringify(b)); };
const probe = (cls) => { const el = document.createElement("div"); el.className = cls; document.body.append(el); const px = getComputedStyle(el).fontSize; el.remove(); return px; };
t("logs are small by default", () => eq([probe("console"), probe("dg-log")], ["11px", "11px"], "sizes"));
t("a size you choose applies to every log, and is saved", () => {
  prefs.tab = "appearance"; show("settings");
  sent.length = 0; document.querySelector('[data-logsize="15"]').click();
  eq([probe("console"), probe("dg-log")], ["15px", "15px"], "sizes");
  eq(sent.filter((x) => x.type === "setpref").map((x) => [x.name, x.value]), [["logSize", 15]], "saved");
  eq(document.querySelector('[data-logsize="15"]').getAttribute("aria-pressed"), "true", "shown as chosen");
});
t("the terminal keeps its own size", () => eq(probe("term") && (() => { const w = document.createElement("div"); w.className = "term"; w.innerHTML = '<div class="console"></div>'; document.body.append(w); const px = getComputedStyle(w.firstChild).fontSize; w.remove(); return px; })(), "12px", "terminal"));
report(R);
