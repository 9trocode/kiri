// Linux: the window opens on the setup checklist until Kiri can run
// containers for this account, fixes each check, and opens only when ready.
const out = [];
const ok = (cond, what) => out.push(`${cond ? "PASS" : "FAIL"} ${what}`);
const row = (k) => document.querySelector(`#onb [data-check="${k}"]`);
const obtn = (sel) => document.querySelector(`#onb ${sel}`);

// A fresh install: engine up, this account not allowed yet.
kiri.on({ type: "onboarding", ready: false, user: "ada", checks: { engine: "ok", running: "ok", access: "no", docker: "off", builds: "off" } });
ok(document.getElementById("onb"), "the checklist covers the window");
ok(row("access").textContent.includes("Lets ada run containers"), "access names the account");
ok(obtn('[data-onbfix="access"]')?.classList.contains("primary"), "the first fix is the primary button");
ok(obtn('[data-onbfix="access"] svg'), "a fix that asks for the password shows a lock");
ok(!obtn('[data-onbfix="docker"]'), "a check that can't be told yet has no fix");
ok(obtn('[data-onb="open"]').disabled, "Open Kiri waits for the required checks");
ok(document.querySelector("#onb .onb-ft").textContent.includes("password once"), "says the password is asked once");

obtn('[data-onbfix="access"]').click();
ok(sent.at(-1)?.type === "onbfix" && sent.at(-1).name === "access", "a fix asks the app to fix that check");
ok(row("access").querySelector(".st.wait"), "the check shows it's being fixed");
ok(obtn('[data-onbfix="access"]') === null || obtn('[data-onbfix="access"]').disabled, "its button can't be pressed twice");

// Engine stopped: one fix covers the engine and access (one password prompt).
kiri.on({ type: "onboarding", ready: false, user: "ada", checks: { engine: "ok", running: "no", access: "off", docker: "off", builds: "off" } });
ok(obtn('[data-onbfix="running"]').textContent.includes("Start engine"), "a stopped engine offers to start it");
obtn('[data-onbfix="running"]').click();
ok(row("running").querySelector(".st.wait") && row("access").querySelector(".st.wait") && !row("docker").querySelector(".st.wait"), "starting the engine also gives access: both spin, nothing else");

// The prompt was cancelled.
kiri.on({ type: "onboarding", ready: false, user: "ada", error: "Not changed: the password prompt was cancelled", checks: { engine: "ok", running: "no", access: "off", docker: "off", builds: "off" } });
ok(document.querySelector("#onb .onb-err")?.textContent.includes("cancelled"), "says why a fix didn't happen");

// No access lists on this system: log out to finish.
kiri.on({ type: "onboarding", ready: false, user: "ada", checks: { engine: "ok", running: "ok", access: "restart", docker: "off", builds: "off" } });
ok(obtn('[data-onb="logout"]') && obtn('[data-onb="restart"]'), "offers to log out or restart when only that will do");
obtn('[data-onb="logout"]').click();
ok(sent.at(-1)?.type === "onblogout", "Log out now asks the app to log out");
ok(obtn('[data-onb="open"]').disabled, "Open Kiri still waits");

// Ready, with docker and builds left: setup isn't complete, so Open Kiri waits.
kiri.on({ type: "onboarding", ready: true, complete: false, user: "ada", checks: { engine: "ok", running: "ok", access: "ok", docker: "no", builds: "no" } });
ok(obtn('[data-onbfix="all"]'), "Fix all when more than one is left");
ok(obtn('[data-onb="open"]').disabled, "Open Kiri waits until docker and builds are set up or skipped");
ok(obtn('[data-onbskip="docker"]') && obtn('[data-onbskip="builds"]'), "docker and builds can be skipped");
ok(!obtn('[data-onbskip="running"]') && !obtn('[data-onbskip="access"]'), "the engine and access can't be skipped");
obtn('[data-onbskip="docker"]').click();
ok(sent.at(-1)?.type === "onbskip" && sent.at(-1)?.name === "docker" && sent.at(-1)?.value === true, "Skip asks the app to skip docker");

// docker skipped: it says so, offers Undo, and drops out of Fix all.
kiri.on({ type: "onboarding", ready: true, complete: false, skipped: ["docker"], user: "ada", checks: { engine: "ok", running: "ok", access: "ok", docker: "no", builds: "no" } });
ok(/Skipped/.test(document.querySelector('[data-check="docker"]').textContent), "a skipped check says it was skipped");
ok(obtn('[data-onbskip="docker"][data-undo]'), "and offers Undo");
ok(!obtn('[data-onbfix="docker"]') && !obtn('[data-onbfix="all"]'), "no Set up or Fix all for what was skipped");
ok(obtn('[data-onb="open"]').disabled, "Open Kiri still waits for builds");

// Builds set up too: complete.
kiri.on({ type: "onboarding", ready: true, complete: true, skipped: ["docker"], user: "ada", checks: { engine: "ok", running: "ok", access: "ok", docker: "no", builds: "ok" } });
ok(!obtn('[data-onb="open"]').disabled, "Open Kiri once setup is complete");
obtn('[data-onb="open"]').click();
ok(!document.getElementById("onb") && sent.at(-1)?.type === "onbopen", "Open Kiri shows the window and starts it");

report(out);
