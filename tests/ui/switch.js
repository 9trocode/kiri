// Switching docker to Kiri: the strip, the dialog, choosing what to bring,
// and Switch back. Run with scripts/ui-test.py.
const inv=(big)=>({type:"inventory",source:"orbstack",images:[{name:"grafana/grafana:latest",size:big?"40GB":"712MB",have:false},{name:"redis:7",size:"138MB",have:false},{name:"nginx:alpine",size:"52MB",have:true}],volumes:[{name:"pgdata",anonymous:false,have:false}],containers:[]});
const reset=(big=false)=>{sent.length=0;ctx.card=null;ctx.pending=null;document.querySelectorAll(".scrim").forEach(x=>x.remove());if(impModal)closeImportModal();ui.imp.running=false;for(const k of Object.keys(ui.imp.picked))ui.imp.picked[k].clear();
  kiri.on({type:"status",version:"0.1.4",dev:true,app:"Kiri Dev",ours:"kiri-dev",context:"orbstack"});
  kiri.on({type:"importsources",sources:[{id:"orbstack",label:"OrbStack",running:true}]});kiri.on(inv(big));sent.length=0;};
const click=(sel)=>{const el=document.querySelector(sel);if(!el)throw new Error("no "+sel);el.click();};
const used=()=>sent.filter(x=>x.type==="usecontext");
const R=[];const t=(name,fn)=>{try{fn();R.push("PASS "+name)}catch(e){R.push("FAIL "+name+": "+e.message)}};
const eq=(a,b,m)=>{if(JSON.stringify(a)!==JSON.stringify(b))throw new Error(m+": got "+JSON.stringify(a)+" want "+JSON.stringify(b))};
show("containers");
t("strip and status pill offer the switch",()=>{reset();render();eq(!!document.querySelector("#impstrip [data-ctxswitch]"),true,"strip");eq(!!document.querySelector("#statusbar [data-ctxswitch]"),true,"pill");});
t("Not now switches nothing",()=>{reset();ctxDialog();click("#ctx-dlg [data-close]");eq(used().length,0,"usecontext");eq(!!document.getElementById("ctx-dlg"),false,"dialog closed");});
t("Choose what to bring does not switch",()=>{reset();ctxDialog();click("[data-ctxchoose]");eq(used().length,0,"usecontext");eq(!!impModal,true,"import window open");eq(impModal.querySelector("[data-impgo]").textContent.startsWith("Switch and import"),true,"button says Switch and import");});
t("closing the chooser switches nothing",()=>{reset();ctxDialog();click("[data-ctxchoose]");closeImportModal();eq(used().length,0,"usecontext");eq(ctx.pending,null,"pending cleared");eq(ctx.card,null,"no card");});
t("importing from the chooser switches, after the import starts",()=>{reset();ctxDialog();click("[data-ctxchoose]");impModal.querySelector("[data-impgo]").click();eq(sent.map(x=>x.type).filter(x=>x==="import"||x==="usecontext"),["import","usecontext"],"order");eq(used()[0].name,"kiri-dev","target");eq(ctx.card.total>0,true,"card counts");});
t("Switch with the box ticked brings every image",()=>{reset();ctxDialog();eq(document.getElementById("ctx-bring").checked,true,"ticked when small");click("[data-ctxgo]");const im=sent.find(x=>x.type==="import");eq(im.value.images,["grafana/grafana:latest","redis:7"],"images");eq(used()[0].name,"kiri-dev","switched");});
t("Switch with the box unticked only switches",()=>{reset();ctxDialog();document.getElementById("ctx-bring").checked=false;click("[data-ctxgo]");eq(sent.some(x=>x.type==="import"),false,"no import");eq(used().length,1,"switched");eq(ctx.card.total,0,"card without import");});
t("a big import is not ticked for you",()=>{reset(true);ctxDialog();eq(document.getElementById("ctx-bring").checked,false,"unticked over 10 GB");});
t("progress counts each finished item",()=>{reset();ctxDialog();click("[data-ctxgo]");kiri.on({type:"importstep",kind:"images",name:"redis:7",state:"done"});eq(ctx.card.done,1,"done");eq(document.getElementById("ctxcard").textContent.includes("1 of 2"),true,"card shows 1 of 2");});
t("Switch back restores the previous engine",()=>{reset();ctxDialog();click("[data-ctxgo]");sent.length=0;click("#ctxcard [data-ctxback]");eq(used()[0].name,"orbstack","back to orbstack");eq(ctx.card,null,"card gone");});
report(R);
