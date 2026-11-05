// The calligraphy scenes: each one draws its background as SVG (1600x900, text column on the left 62%).
// Exported to assets/calligraphy by tools/calligraphy/export.py.
const W=1600,H=900,CX=496; // the calligraphy sits in the left 62%
let uid=0;const U=p=>p+(uid++);
const star=(cx,cy,R,r,n=8,rot=-Math.PI/2)=>{let p=[];for(let i=0;i<n*2;i++){const a=Math.PI*i/n+rot,q=i%2?r:R;p.push((cx+q*Math.cos(a)).toFixed(1)+","+(cy+q*Math.sin(a)).toFixed(1))}return p.join(" ")};
const rnd=(s=>()=>(s=(s*9301+49297)%233280)/233280);
const grain=(id,op=.18,freq=.8)=>`<filter id="${id}"><feTurbulence type="fractalNoise" baseFrequency="${freq}" numOctaves="2" stitchTiles="stitch"/><feColorMatrix values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 ${op} 0"/></filter>`;
const svg=(defs,body)=>`<svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="xMidYMid slice" xmlns="http://www.w3.org/2000/svg"><defs>${defs}</defs>${body}</svg>`;
const archPath=(x,y,w,h)=>{const r=w/2;return `M${x} ${y+h}V${y+r*0.9}C${x} ${y+r*.35} ${x+r*.55} ${y+r*.12} ${x+r} ${y}C${x+r*1.45} ${y+r*.12} ${x+w} ${y+r*.35} ${x+w} ${y+r*.9}V${y+h}Z`};
const girihTile=(id,c,sw=1.2,t=110)=>{const h=t/2;return `<pattern id="${id}" width="${t}" height="${t}" patternUnits="userSpaceOnUse"><polygon points="${star(h,h,h*.92,h*.62)}" fill="none" stroke="${c}" stroke-width="${sw}"/><polygon points="${star(h,h,h*.48,h*.34)}" fill="none" stroke="${c}" stroke-width="${sw*.7}"/><path d="M0 0L${h*.35} ${h*.35}M${t} 0L${t-h*.35} ${h*.35}M0 ${t}L${h*.35} ${t-h*.35}M${t} ${t}L${t-h*.35} ${t-h*.35}" stroke="${c}" stroke-width="${sw}"/></pattern>`};
const mosque=(y,c,s=1,x0=0)=>{ // skyline: domes + minarets
  const dome=(x,w,h)=>`M${x} ${y}V${y-h*.25}Q${x} ${y-h} ${x+w/2} ${y-h}Q${x+w} ${y-h} ${x+w} ${y-h*.25}V${y}Z M${x+w/2-2} ${y-h}V${y-h-18*s}H${x+w/2+2}V${y-h}Z`;
  const min=(x,h)=>`M${x} ${y}V${y-h}L${x+7*s} ${y-h-30*s}L${x+14*s} ${y-h}V${y}Z M${x-3*s} ${y-h*.6}H${x+17*s}V${y-h*.6+6*s}H${x-3*s}Z`;
  return `<path fill="${c}" d="${min(x0+120*s,260*s)} ${dome(x0+170*s,150*s,120*s)} ${dome(x0+310*s,260*s,210*s)} ${dome(x0+560*s,150*s,120*s)} ${min(x0+720*s,260*s)} M${x0+140*s} ${y}V${y-70*s}H${x0+740*s}V${y}Z"/>`};


// a soft wash on the clock side so its text stays readable on light scenes
const veil=c=>{const id=U("v");return `<linearGradient id="${id}" x1="0" x2="1"><stop offset="0" stop-color="${c}" stop-opacity="0"/><stop offset=".14" stop-color="${c}" stop-opacity=".86"/><stop offset="1" stop-color="${c}" stop-opacity=".9"/></linearGradient><rect x="930" width="670" height="900" fill="url(#${id})"/>`};
// a hanging fanous lantern with its glow
const fanous=(x,top,s,gl)=>`<line x1="${x}" y1="${top-200}" x2="${x}" y2="${top}" stroke="#c9a24e" stroke-width="1.5" opacity=".8"/><circle cx="${x}" cy="${top+60*s}" r="${110*s}" fill="url(#${gl})"/><path d="M${x-26*s} ${top+18*s}H${x+26*s}L${x+36*s} ${top+44*s}L${x+28*s} ${top+104*s}L${x} ${top+124*s}L${x-28*s} ${top+104*s}L${x-36*s} ${top+44*s}Z" fill="#f6cf74" stroke="#8a6420" stroke-width="${2.4*s}"/><path d="M${x} ${top+26*s}V${top+112*s}M${x-30*s} ${top+64*s}H${x+30*s}" stroke="#8a6420" stroke-width="${1.6*s}" opacity=".7"/><path d="M${x-14*s} ${top}H${x+14*s}L${x+26*s} ${top+18*s}H${x-26*s}Z M${x-2*s} ${top+124*s}H${x+2*s}V${top+140*s}H${x-2*s}Z" fill="#8a6420"/>`;
// old-city rooftops with domes, minarets and lit windows
const city=(base,c,seed,lit)=>{const q=rnd(seed);let d="",win="";let x=-20;while(x<1700){const w=50+q()*90,h=60+q()*130;d+=`M${x.toFixed(0)} ${base}V${(base-h).toFixed(0)}H${(x+w).toFixed(0)}V${base}Z`;
  if(q()>.6){const r=w*.35;d+=`M${(x+w/2-r).toFixed(0)} ${(base-h).toFixed(0)}A${r.toFixed(0)} ${(r*1.1).toFixed(0)} 0 0 1 ${(x+w/2+r).toFixed(0)} ${(base-h).toFixed(0)}Z`}
  if(q()>.85){d+=`M${(x+w*.7).toFixed(0)} ${(base-h).toFixed(0)}V${(base-h-90).toFixed(0)}L${(x+w*.7+6).toFixed(0)} ${(base-h-110).toFixed(0)}L${(x+w*.7+12).toFixed(0)} ${(base-h-90).toFixed(0)}V${(base-h).toFixed(0)}Z`}
  for(let k=0;k<3;k++)if(q()>.55)win+=`<rect x="${(x+8+q()*(w-24)).toFixed(0)}" y="${(base-h+16+q()*(h-40)).toFixed(0)}" width="8" height="12" rx="4" fill="${lit}" opacity="${(.6+q()*.4).toFixed(2)}"/>`;
  x+=w+2}return `<path d="${d}" fill="${c}"/>${win}`};
// a string of pennant flags sagging between two points
const bunting=(x1,y1,x2,y2,c1,c2,c3)=>{const cx=(x1+x2)/2,cy=Math.max(y1,y2)+60;let s=`<path d="M${x1} ${y1}Q${cx} ${cy} ${x2} ${y2}" fill="none" stroke="#7a5a3a" stroke-width="2"/>`;const cs=[c1,c2,c3];
  for(let i=1;i<16;i++){const t=i/16,px=(1-t)*(1-t)*x1+2*(1-t)*t*cx+t*t*x2,py=(1-t)*(1-t)*y1+2*(1-t)*t*cy+t*t*y2;s+=`<polygon points="${(px-18).toFixed(0)},${py.toFixed(0)} ${(px+18).toFixed(0)},${py.toFixed(0)} ${px.toFixed(0)},${(py+42).toFixed(0)}" fill="${cs[i%3]}"/>`}return s};

// ---------- real architecture, drawn as silhouettes with lit details ----------
const N=v=>(+v).toFixed(1);
// a dome sitting on y: half ellipse r wide, h tall
const domeP=(x,y,r,h)=>`M${N(x-r)} ${N(y)}A${N(r)} ${N(h)} 0 0 1 ${N(x+r)} ${N(y)}Z`;
// a crescent finial (alem) on a pole
const alem=(x,y,s,c)=>`<path d="M${N(x)} ${N(y)}V${N(y-26*s)}" stroke="${c}" stroke-width="${N(2.2*s)}"/><circle cx="${N(x)}" cy="${N(y-12*s)}" r="${N(3.5*s)}" fill="${c}"/><path d="M${N(x-6*s)} ${N(y-34*s)}A${N(8*s)} ${N(8*s)} 0 1 0 ${N(x+6*s)} ${N(y-34*s)}A${N(6*s)} ${N(7*s)} 0 1 1 ${N(x-6*s)} ${N(y-34*s)}Z" fill="${c}"/>`;
// small round-topped windows in a row
const wins=(x0,x1,y,w,h,n,c,op=1)=>{let s="";for(let i=0;i<n;i++){const x=x0+(x1-x0)*(i+.5)/n;s+=`<path d="M${N(x-w/2)} ${N(y+h)}V${N(y+w/2)}A${N(w/2)} ${N(w/2)} 0 0 1 ${N(x+w/2)} ${N(y+w/2)}V${N(y+h)}Z" fill="${c}" opacity="${op}"/>`}return s};
// a pointed (Islamic) arch opening
const ptArch=(x,y,w,h)=>`M${N(x)} ${N(y+h)}V${N(y+w*.55)}Q${N(x)} ${N(y+w*.12)} ${N(x+w/2)} ${N(y)}Q${N(x+w)} ${N(y+w*.12)} ${N(x+w)} ${N(y+w*.55)}V${N(y+h)}Z`;

// Ottoman, like the Blue Mosque: one great dome over a drum, half-domes, corner domes,
// weight turrets and four pencil minarets with balconies.
const ottoman=(cx,b,s,f,lit)=>{let p="",d="";
  const pencil=(x,h)=>{const top=b-h*s;p+=`M${N(x-15*s)} ${b}V${N(b-70*s)}H${N(x+15*s)}V${b}Z M${N(x-9*s)} ${N(b-70*s)}V${N(top)}H${N(x+9*s)}V${N(b-70*s)}Z`;
    [.5,.72,.9].forEach(k=>{const y=b-h*s*k;p+=`M${N(x-17*s)} ${N(y)}H${N(x+17*s)}L${N(x+12*s)} ${N(y+9*s)}H${N(x-12*s)}Z`});
    p+=`M${N(x-10*s)} ${N(top)}L${N(x)} ${N(top-78*s)}L${N(x+10*s)} ${N(top)}Z`;d+=alem(x,top-78*s,.6*s,f)};
  p+=`M${N(cx-300*s)} ${b}V${N(b-140*s)}H${N(cx+300*s)}V${b}Z`;
  [-1,1].forEach(k=>{p+=domeP(cx+k*240*s,b-140*s,52*s,44*s)+domeP(cx+k*140*s,b-140*s,92*s,80*s);
    p+=`M${N(cx+k*175*s-8*s)} ${N(b-140*s)}V${N(b-238*s)}H${N(cx+k*175*s+8*s)}V${N(b-140*s)}Z`+domeP(cx+k*175*s,b-238*s,10*s,14*s)});
  p+=`M${N(cx-150*s)} ${N(b-140*s)}V${N(b-212*s)}H${N(cx+150*s)}V${N(b-140*s)}Z`+domeP(cx,b-212*s,152*s,138*s);
  d+=alem(cx,b-350*s,1.1*s,f);
  pencil(cx-335*s,430);pencil(cx+335*s,430);pencil(cx-268*s,360);pencil(cx+268*s,360);
  let w=wins(cx-140*s,cx+140*s,b-205*s,9*s,22*s,13,lit,.9)+wins(cx-290*s,cx+290*s,b-118*s,14*s,34*s,14,lit,.85)+wins(cx-290*s,cx+290*s,b-62*s,14*s,34*s,14,lit,.7);
  w+=`<path d="${ptArch(cx-34*s,b-96*s,68*s,96*s)}" fill="${lit}" opacity=".95"/>`;
  return `<path d="${p}" fill="${f}"/>${d}${w}`};

// Mughal, like the Badshahi Mosque: a raised platform, a tall iwan with a pointed arch,
// three marble onion domes on drums, corner chhatris and tapering minarets with kiosks.
const mughal=(cx,b,s,f,marble,lit)=>{let p="",m="",d="";
  const onion=(x,y,w,h)=>`M${N(x-w*.36)} ${N(y)}C${N(x-w*.62)} ${N(y-h*.38)} ${N(x-w*.52)} ${N(y-h*.74)} ${N(x)} ${N(y-h)}C${N(x+w*.52)} ${N(y-h*.74)} ${N(x+w*.62)} ${N(y-h*.38)} ${N(x+w*.36)} ${N(y)}Z`;
  const chhatri=(x,y,w,c)=>{let q=`M${N(x-w/2)} ${N(y)}H${N(x+w/2)}V${N(y-6*s)}H${N(x-w/2)}Z`;[-.42,-.14,.14,.42].forEach(k=>q+=`M${N(x+k*w-2.2*s)} ${N(y-6*s)}V${N(y-36*s)}H${N(x+k*w+2.2*s)}V${N(y-6*s)}Z`);q+=`M${N(x-w*.62)} ${N(y-36*s)}H${N(x+w*.62)}L${N(x+w*.5)} ${N(y-42*s)}H${N(x-w*.5)}Z`;return {q,top:onion(x,y-42*s,w*.95,w*.95)}};
  p+=`M${N(cx-400*s)} ${b}V${N(b-34*s)}H${N(cx+400*s)}V${b}Z M${N(cx-300*s)} ${N(b-34*s)}V${N(b-220*s)}H${N(cx+300*s)}V${N(b-34*s)}Z M${N(cx-96*s)} ${N(b-34*s)}V${N(b-300*s)}H${N(cx+96*s)}V${N(b-34*s)}Z`;
  [-1,1].forEach(k=>{const x=cx+k*395*s,top=b-470*s;p+=`M${N(x-20*s)} ${b}L${N(x-14*s)} ${N(top)}H${N(x+14*s)}L${N(x+20*s)} ${b}Z`;[.35,.62,.86].forEach(t=>{const y=b-470*s*t;p+=`M${N(x-26*s)} ${N(y)}H${N(x+26*s)}L${N(x+20*s)} ${N(y+8*s)}H${N(x-20*s)}Z`});
    const c=chhatri(x,top,34*s);p+=c.q;m+=c.top;d+=alem(x,top-74*s,.55*s,marble);
    const c2=chhatri(cx+k*300*s,b-220*s,40*s);p+=c2.q;m+=c2.top;d+=alem(cx+k*300*s,b-300*s,.5*s,marble);
    p+=`M${N(cx+k*210*s-48*s)} ${N(b-220*s)}V${N(b-250*s)}H${N(cx+k*210*s+48*s)}V${N(b-220*s)}Z`;m+=onion(cx+k*210*s,b-250*s,124*s,126*s);d+=alem(cx+k*210*s,b-376*s,.7*s,marble)});
  p+=`M${N(cx-72*s)} ${N(b-300*s)}V${N(b-334*s)}H${N(cx+72*s)}V${N(b-300*s)}Z`;m+=onion(cx,b-334*s,190*s,190*s);d+=alem(cx,b-524*s,1*s,marble);
  let w=`<path d="${ptArch(cx-62*s,b-268*s,124*s,234*s)}" fill="${lit}" opacity=".9"/>`;
  [-1,1].forEach(k=>{for(let i=0;i<2;i++){const x=cx+k*(140+i*70)*s-22*s;w+=`<path d="${ptArch(x,b-200*s,44*s,70*s)}" fill="${lit}" opacity=".75"/><path d="${ptArch(x,b-116*s,44*s,72*s)}" fill="${lit}" opacity=".75"/>`}});
  return `<path d="${p}" fill="${f}"/><path d="${m}" fill="${marble}"/>${d}${w}`};

// Moroccan, like the Koutoubia: a square minaret with blind arches and merlons, a small
// lantern tower with brass balls (jamour), a long hall under green tiled roofs.
const moroccan=(cx,b,s,f,roof,lit)=>{let p="",r="",d="";const mx=cx-150*s;
  p+=`M${N(cx-60*s)} ${b}V${N(b-120*s)}H${N(cx+380*s)}V${b}Z`;
  for(let i=0;i<5;i++){const x=cx-50*s+i*86*s;r+=`M${N(x)} ${N(b-120*s)}L${N(x+43*s)} ${N(b-160*s)}L${N(x+86*s)} ${N(b-120*s)}Z`}
  p+=`M${N(cx-420*s)} ${b}V${N(b-70*s)}H${N(cx-60*s)}V${b}Z`;for(let i=0;i<14;i++){const x=cx-420*s+i*26*s;p+=`M${N(x+4*s)} ${N(b-70*s)}V${N(b-84*s)}H${N(x+18*s)}V${N(b-70*s)}Z`}
  p+=`M${N(mx-50*s)} ${b}V${N(b-430*s)}H${N(mx+50*s)}V${b}Z`;for(let i=0;i<5;i++){const x=mx-50*s+i*22*s;p+=`M${N(x+2*s)} ${N(b-430*s)}V${N(b-448*s)}L${N(x+9*s)} ${N(b-456*s)}L${N(x+16*s)} ${N(b-448*s)}V${N(b-430*s)}Z`}
  p+=`M${N(mx-20*s)} ${N(b-430*s)}V${N(b-500*s)}H${N(mx+20*s)}V${N(b-430*s)}Z`+domeP(mx,b-500*s,22*s,18*s);
  d+=`<path d="M${N(mx)} ${N(b-518*s)}V${N(b-580*s)}" stroke="#d9a84c" stroke-width="${N(2.5*s)}"/><circle cx="${N(mx)}" cy="${N(b-530*s)}" r="${N(7*s)}" fill="#d9a84c"/><circle cx="${N(mx)}" cy="${N(b-548*s)}" r="${N(5.5*s)}" fill="#d9a84c"/><circle cx="${N(mx)}" cy="${N(b-563*s)}" r="${N(4*s)}" fill="#d9a84c"/>`;
  let w="";[[-310,90],[-230,170],[-130,250],[-30,330]].forEach(([dy,h])=>{});
  [b-380*s,b-290*s,b-200*s].forEach((y,k)=>{w+=`<path d="${ptArch(mx-30*s,y,24*s,50*s)}" fill="${lit}" opacity=".7"/><path d="${ptArch(mx+6*s,y,24*s,50*s)}" fill="${lit}" opacity=".7"/>`});
  w+=`<path d="M${N(mx-14*s)} ${N(b-470*s)}h${N(28*s)}v${N(22*s)}h${N(-28*s)}z" fill="${lit}" opacity=".7"/>`;
  w+=`<path d="M${N(cx-270*s)} ${b}V${N(b-38*s)}A${N(22*s)} ${N(22*s)} 0 1 1 ${N(cx-226*s)} ${N(b-38*s)}V${b}Z" fill="${lit}" opacity=".8"/>`+wins(cx-40*s,cx+370*s,b-96*s,18*s,52*s,8,lit,.7);
  return `<path d="${p}" fill="${f}"/><path d="${r}" fill="${roof}"/>${d}${w}`};

// Mamluk, like Sultan Qaitbay in Cairo: a ribbed dome on a tall drum, a recessed portal,
// cresting along the roof, and a minaret that goes square, octagonal, round, then a pavilion.
const mamluk=(cx,b,s,f,lit,rib)=>{let p="",d="";
  p+=`M${N(cx-280*s)} ${b}V${N(b-150*s)}H${N(cx+230*s)}V${b}Z`;for(let i=0;i<26;i++){const x=cx-280*s+i*19.6*s;p+=domeP(x+9.8*s,b-150*s,7*s,12*s)}
  p+=`M${N(cx-70*s)} ${b}V${N(b-230*s)}H${N(cx+10*s)}V${b}Z`;
  const dx=cx+120*s;p+=`M${N(dx-92*s)} ${N(b-150*s)}V${N(b-270*s)}H${N(dx+92*s)}V${N(b-150*s)}Z`;
  const dw=176*s,dh=200*s,dy=b-270*s;p+=`M${N(dx-dw/2)} ${N(dy)}C${N(dx-dw/2)} ${N(dy-dh*.6)} ${N(dx-dw*.12)} ${N(dy-dh*.86)} ${N(dx)} ${N(dy-dh)}C${N(dx+dw*.12)} ${N(dy-dh*.86)} ${N(dx+dw/2)} ${N(dy-dh*.6)} ${N(dx+dw/2)} ${N(dy)}Z`;
  let ribs="";for(let k=-3;k<=3;k++){const bx=dx+k*dw/7.5;ribs+=`<path d="M${N(bx)} ${N(dy)}Q${N(dx+k*dw/11)} ${N(dy-dh*.62)} ${N(dx)} ${N(dy-dh)}" fill="none" stroke="${rib}" stroke-width="${N(1.6*s)}"/>`}
  d+=alem(dx,dy-dh,.9*s,f);
  const mx=cx-200*s;p+=`M${N(mx-28*s)} ${b}V${N(b-280*s)}H${N(mx+28*s)}V${b}Z`;
  [[b-280*s,40],[b-395*s,32],[b-480*s,26]].forEach(([y,hw])=>p+=`M${N(mx-hw*s)} ${N(y)}H${N(mx+hw*s)}L${N(mx+hw*s-6*s)} ${N(y+10*s)}H${N(mx-hw*s+6*s)}Z`);
  p+=`M${N(mx-20*s)} ${N(b-280*s)}L${N(mx-20*s)} ${N(b-395*s)}H${N(mx+20*s)}V${N(b-280*s)}Z M${N(mx-14*s)} ${N(b-395*s)}V${N(b-480*s)}H${N(mx+14*s)}V${N(b-395*s)}Z`;
  [-10,-3.5,3.5,10].forEach(k=>p+=`M${N(mx+k*s-1.6*s)} ${N(b-480*s)}V${N(b-515*s)}H${N(mx+k*s+1.6*s)}V${N(b-480*s)}Z`);
  p+=`M${N(mx-14*s)} ${N(b-515*s)}C${N(mx-20*s)} ${N(b-532*s)} ${N(mx-6*s)} ${N(b-548*s)} ${N(mx)} ${N(b-556*s)}C${N(mx+6*s)} ${N(b-548*s)} ${N(mx+20*s)} ${N(b-532*s)} ${N(mx+14*s)} ${N(b-515*s)}Z`;d+=alem(mx,b-556*s,.55*s,f);
  let w=`<path d="${ptArch(cx-56*s,b-210*s,52*s,170*s)}" fill="${lit}" opacity=".85"/>`+wins(cx+30*s,cx+215*s,b-120*s,14*s,40*s,6,lit,.75)+wins(cx-265*s,cx-90*s,b-120*s,14*s,40*s,5,lit,.75)+wins(dx-80*s,dx+80*s,b-252*s,10*s,26*s,7,lit,.8);
  [b-250*s,b-360*s].forEach(y=>w+=`<rect x="${N(mx-5*s)}" y="${N(y)}" width="${N(10*s)}" height="${N(22*s)}" rx="${N(5*s)}" fill="${lit}" opacity=".8"/>`);
  return `<path d="${p}" fill="${f}"/>${ribs}${d}${w}`};

// Bengal sultanate, like the Shat Gombuj Mosque: a long low hall with a curved cornice,
// rows of small domes, round corner towers with bands, and a row of arched doors.
const bengal=(cx,b,s,f,lit,band)=>{let p="",d="";const L=cx-380*s,R=cx+380*s;
  p+=`M${N(L)} ${b}V${N(b-130*s)}Q${N(cx)} ${N(b-152*s)} ${N(R)} ${N(b-130*s)}V${b}Z`;
  for(let i=0;i<11;i++){const x=L+40*s+i*(R-L-80*s)/10;p+=domeP(x,b-138*s+Math.abs(i-5)*.8*s,26*s,30*s)}
  p+=`M${N(cx-46*s)} ${N(b-150*s)}Q${N(cx)} ${N(b-205*s)} ${N(cx+46*s)} ${N(b-150*s)}Z`;
  [L,R].forEach(x=>{p+=`M${N(x-30*s)} ${b}V${N(b-172*s)}Q${N(x)} ${N(b-180*s)} ${N(x+30*s)} ${N(b-172*s)}V${b}Z`+domeP(x,b-176*s,24*s,26*s);[.25,.5,.75].forEach(k=>d+=`<rect x="${N(x-30*s)}" y="${N(b-178*s*k)}" width="${N(60*s)}" height="${N(3*s)}" fill="${band}"/>`)});
  let w="";for(let i=0;i<11;i++){const x=L+40*s+i*(R-L-80*s)/10;w+=`<path d="${ptArch(x-15*s,b-84*s,30*s,84*s)}" fill="${lit}" opacity="${i==5?.95:.75}"/>`}
  return `<path d="${p}" fill="${f}"/>${d}${w}`};

// a South Asian eidgah: a long wall with a raised central mihrab, kangura cresting and two
// small minaret turrets with domes.
const eidgah=(cx,b,s,f,lit)=>{let p="";const L=cx-330*s,R=cx+330*s;
  p+=`M${N(L)} ${b}V${N(b-70*s)}H${N(R)}V${b}Z`;for(let x=L;x<R-4;x+=20*s)p+=domeP(x+10*s,b-70*s,8*s,12*s);
  p+=`M${N(cx-90*s)} ${b}V${N(b-150*s)}H${N(cx+90*s)}V${b}Z`;for(let x=cx-90*s;x<cx+86*s;x+=20*s)p+=domeP(x+10*s,b-150*s,8*s,12*s);
  [-1,1].forEach(k=>{const x=cx+k*112*s;p+=`M${N(x-14*s)} ${b}V${N(b-200*s)}H${N(x+14*s)}V${b}Z`+`M${N(x-20*s)} ${N(b-200*s)}H${N(x+20*s)}V${N(b-208*s)}H${N(x-20*s)}Z`+domeP(x,b-208*s,18*s,28*s)});
  const x0=cx-46*s;let w=`<path d="${ptArch(x0,b-130*s,92*s,130*s)}" fill="${lit}"/>`;
  [-1,1].forEach(k=>{for(let i=1;i<=4;i++){const x=cx+k*(110+i*46)*s;w+=`<path d="${ptArch(x-14*s,b-56*s,28*s,46*s)}" fill="${lit}" opacity=".7"/>`}});
  return `<path d="${p}" fill="${f}"/>${w}`};

const D=[
 {n:"Illuminated page",tag:"pick",ar:["لا إله إلا الله","محمد رسول الله"],en:"There is no god but Allah, Muhammad is the Messenger of Allah",ref:"The testimony of faith",f:"RK",ink:"#3a2a12",pt:"#2a1f10",gold:"#8a5f1c",
  bg(){const g=U("g"),p=U("p"),gr=U("n");const f=`<rect x="150" y="150" width="692" height="600" fill="none" stroke="#9a7230" stroke-width="2.4"/><rect x="164" y="164" width="664" height="572" fill="none" stroke="#9a7230" stroke-width="1"/>`;
   const corner=(x,y)=>`<polygon points="${star(x,y,30,20)}" fill="#c9a24e" stroke="#7d5a1d" stroke-width="1.2"/><circle cx="${x}" cy="${y}" r="7" fill="#1f4f7a"/>`;
   return svg(`<radialGradient id="${g}" cx="30%" cy="45%" r="80%"><stop offset="0" stop-color="#f6ecd6"/><stop offset=".7" stop-color="#e7d4ad"/><stop offset="1" stop-color="#d2b986"/></radialGradient>${girihTile(p,"#b08a45",.8,72)}${grain(gr,.22)}`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/><rect x="164" y="164" width="664" height="572" fill="url(#${p})" opacity=".13"/><rect x="190" y="190" width="612" height="520" fill="#f6ecd6" opacity=".55"/>${f}${corner(150,150)+corner(842,150)+corner(150,750)+corner(842,750)}<rect width="${W}" height="${H}" filter="url(#${gr})"/>`)},box:[190,190,802,710]},
 {n:"Night over the mosque",tag:"pick",ar:["ألا بذكر الله","تطمئن القلوب"],en:"Verily, in the remembrance of Allah do hearts find rest",ref:"Ar-Ra'd 13:28",f:"NK",ink:"#f4e3b0",pt:"#f6f2ea",gold:"#e9c46a",
  bg(){const g=U("g"),h=U("h"),mk=U("k"),r=rnd(11);let s="";for(let i=0;i<220;i++)s+=`<circle cx="${(r()*W).toFixed(0)}" cy="${(r()*600).toFixed(0)}" r="${(r()*1.5+.3).toFixed(2)}" fill="#fff" opacity="${(r()*.7+.2).toFixed(2)}"/>`;
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#050b1f"/><stop offset=".6" stop-color="#13234a"/><stop offset="1" stop-color="#2a3d66"/></linearGradient><radialGradient id="${h}"><stop offset="0" stop-color="#f4e3b0" stop-opacity=".35"/><stop offset="1" stop-color="#f4e3b0" stop-opacity="0"/></radialGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/>${s}<circle cx="820" cy="150" r="120" fill="url(#${h})"/><mask id="${mk}"><circle cx="820" cy="150" r="58" fill="#fff"/><circle cx="846" cy="134" r="50" fill="#000"/></mask><circle cx="820" cy="150" r="58" fill="#f4e3b0" mask="url(#${mk})"/>${ottoman(CX,900,.9,"#030713","#ffcf6a")}<rect y="896" width="${W}" height="4" fill="#030713"/>`)},box:[160,110,832,470]},
 {n:"Dunes at dawn",tag:"pick",ar:["إن مع العسر","يسرا"],en:"Indeed, with hardship comes ease",ref:"Ash-Sharh 94:6",f:"CH",ink:"#4a1f0c",pt:"#3a1a0e",gold:"#8b3a1c",s:1.15,
  bg(){const g=U("g"),sun=U("s");const dune=(y,amp,c,ph)=>{let d=`M0 ${H}V${y}`;for(let x=0;x<=W;x+=80)d+=` L${x} ${(y+Math.sin(x/260+ph)*amp+Math.sin(x/97+ph*2)*amp*.3).toFixed(1)}`;return `<path d="${d}V${H}Z" fill="${c}"/>`};
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f7d9b5"/><stop offset=".55" stop-color="#f3b98a"/><stop offset="1" stop-color="#e7956a"/></linearGradient><radialGradient id="${sun}"><stop offset="0" stop-color="#fff3d6"/><stop offset=".35" stop-color="#ffe2a8" stop-opacity=".8"/><stop offset="1" stop-color="#ffe2a8" stop-opacity="0"/></radialGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/><circle cx="1100" cy="560" r="300" fill="url(#${sun})"/><circle cx="1100" cy="560" r="70" fill="#fff1d2"/>${dune(600,30,"#e9a77a",1)}${dune(680,40,"#d98a5c",2.4)}${dune(770,35,"#c4703f",4)}${dune(850,25,"#a8572d",5.5)}`)},box:[150,140,840,560]},
 {n:"Lanterns",tag:"pick",ar:["توكلت على الله"],en:"I put my trust in Allah",ref:"Hud 11:56",f:"EM",ink:"#f6e2ae",pt:"#f6f2ea",gold:"#f0c26a",s:1.1,
  bg(){const g=U("g"),gl=U("l");const lan=(x,len,s)=>`<line x1="${x}" y1="0" x2="${x}" y2="${len}" stroke="#c9a24e" stroke-width="1.2" opacity=".7"/><circle cx="${x}" cy="${len+60*s}" r="${80*s}" fill="url(#${gl})"/><path d="M${x-22*s} ${len+18*s}H${x+22*s}L${x+30*s} ${len+40*s}L${x+24*s} ${len+92*s}L${x} ${len+108*s}L${x-24*s} ${len+92*s}L${x-30*s} ${len+40*s}Z" fill="#f3c66d" stroke="#8a6420" stroke-width="${2*s}"/><path d="M${x-12*s} ${len}H${x+12*s}L${x+22*s} ${len+18*s}H${x-22*s}Z M${x-2*s} ${len+108*s}H${x+2*s}V${len+122*s}H${x-2*s}Z" fill="#8a6420"/><path d="M${x} ${len+24*s}V${len+100*s}M${x-26*s} ${len+56*s}H${x+26*s}" stroke="#8a6420" stroke-width="${1.5*s}" opacity=".7"/>`;
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#062b3a"/><stop offset="1" stop-color="#0c4a5e"/></linearGradient><radialGradient id="${gl}"><stop offset="0" stop-color="#ffd88a" stop-opacity=".55"/><stop offset="1" stop-color="#ffd88a" stop-opacity="0"/></radialGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/>${lan(170,180,1)}${lan(330,90,.8)}${lan(660,130,.9)}${lan(820,60,.7)}${lan(520,40,.6)}`)},box:[150,430,842,760]},
 {n:"Tone-on-tone Allah",tag:"pick",ar:["أستغفر الله"],en:"I seek the forgiveness of Allah",ref:"Sahih Muslim 2702",f:"AX",ink:"#24392a",pt:"#1c2a1f",gold:"#5d6d2a",s:1.25,
  bg(){return svg("",`<rect width="${W}" height="${H}" fill="#d9e0cd"/><rect x="0" y="0" width="${W}" height="${H}" fill="none" stroke="#b5c1a0" stroke-width="40"/>`)},box:[150,300,842,620],wm:{t:"الله",x:496,y:640,size:620,c:"#c7d1b6",f:"RK"}},
 {n:"Hexagon tessellation",tag:"pick",ar:["لا حول ولا قوة","إلا بالله"],en:"There is no might nor power except with Allah",ref:"Sahih al-Bukhari 6384",f:"KF",ink:"#3b2a12",pt:"#2a1f10",gold:"#8a5f1c",
  bg(){const p=U("p"),v=U("v");const s=46,h=s*Math.sqrt(3);
   const cube=`<pattern id="${p}" width="${s*3}" height="${h}" patternUnits="userSpaceOnUse">${[[0,0],[s*1.5,h/2],[s*3,0],[0,h],[s*3,h]].map(([x,y])=>`<path d="M${x} ${y}l${s/2} ${-h/2}h${s}l${s/2} ${h/2}l${-s/2} ${h/2}h${-s}z" fill="none" stroke="#a78349" stroke-width="1.1"/><path d="M${x+s/2} ${y-h/2}l${s/2} ${h/2}l${s/2} ${-h/2}M${x+s} ${y}v${h/2}" stroke="#a78349" stroke-width=".7" fill="none"/>`).join("")}</pattern>`;
   return svg(cube+`<radialGradient id="${v}" cx="${CX/W}" cy=".5" r=".45"><stop offset="0" stop-color="#f1e6cc"/><stop offset=".55" stop-color="#f1e6cc" stop-opacity=".9"/><stop offset="1" stop-color="#f1e6cc" stop-opacity="0"/></radialGradient>`,`<rect width="${W}" height="${H}" fill="#e8d8b4"/><rect width="${W}" height="${H}" fill="url(#${p})" opacity=".55"/><rect width="${W}" height="${H}" fill="url(#${v})"/>`)},box:[150,180,842,720]},
 {n:"Ink & seal",tag:"pick",ar:["فاذكروني","أذكركم"],en:"So remember Me; I will remember you",ref:"Al-Baqarah 2:152",f:"LZ",ink:"#1a1714",pt:"#1a1714",gold:"#a8322a",s:1.15,
  bg(){const n=U("n"),b=U("b");
   return svg(grain(n,.25,.65)+`<filter id="${b}"><feTurbulence baseFrequency=".02" numOctaves="3" seed="9"/><feDisplacementMap in="SourceGraphic" scale="18"/></filter>`,
   `<rect width="${W}" height="${H}" fill="#f5f0e4"/><path d="M180 760C320 700 640 735 820 770" stroke="#1a1714" stroke-width="22" stroke-linecap="round" opacity=".12" filter="url(#${b})"/><g transform="translate(760 640)"><rect width="74" height="74" rx="4" fill="#a8322a"/><path d="M12 12h50v12h-38v10h38v28h-50v-12h38v-4h-38z" fill="#f5f0e4"/></g><rect width="${W}" height="${H}" filter="url(#${n})"/>`)},box:[150,160,842,680]},
 {n:"Zellige corner",tag:"pick",ar:["ما شاء الله","لا قوة إلا بالله"],en:"What Allah wills; there is no power except with Allah",ref:"Al-Kahf 18:39",f:"NK",ink:"#5a2614",pt:"#3a1a0e",gold:"#1f6f5c",
  bg(){const cols=["#1f6f5c","#c0573a","#e3b04b","#1f4f7a","#f3ead8"];let tiles="";const r=rnd(3);
   for(let ring=0;ring<6;ring++){const R=90+ring*62;for(let k=0;k<8+ring*4;k++){const a=k/(8+ring*4)*Math.PI/2;tiles+=`<polygon points="${star(Math.cos(a)*R,Math.sin(a)*R,26,14)}" fill="${cols[(ring+k)%4]}" stroke="#f3ead8" stroke-width="3"/>`}}
   const quarter=t=>`<g transform="${t}"><circle r="430" fill="#f3ead8"/><circle r="440" fill="none" stroke="#1f6f5c" stroke-width="8"/>${tiles}<polygon points="${star(0,0,60,34)}" fill="#c0573a" stroke="#f3ead8" stroke-width="4"/></g>`;
   return svg("",`<rect width="${W}" height="${H}" fill="#f1e4d0"/>${quarter("translate(0 0)")}${quarter("translate(992 900) rotate(180)")}`)},box:[180,220,842,700]},
 {n:"Oasis at sunset",tag:"pick",ar:["الله لطيف","بعباده"],en:"Allah is Subtle with His servants",ref:"Ash-Shura 42:19",f:"EM",ink:"#3b1d0e",pt:"#2a160c",gold:"#9a4a1c",s:1.1,
  bg(){const g=U("g"),wv=U("w");
   const palm=(x,y,h,lean)=>{let f="";const tx=x+lean,ty=y-h;[[-160,-20],[-120,40],[-60,70],[0,80],[70,60],[130,30],[170,-10]].forEach(([dx,dy])=>f+=`<path d="M${tx} ${ty}Q${tx+dx*.5} ${ty-50} ${tx+dx} ${ty+dy}" fill="none" stroke="#2b1a10" stroke-width="12" stroke-linecap="round"/>`);
     return `<path d="M${x} ${y}Q${x+lean*.2} ${y-h*.6} ${tx} ${ty}" fill="none" stroke="#2b1a10" stroke-width="16" stroke-linecap="round"/>${f}`};
   let refl="";for(let i=0;i<14;i++)refl+=`<rect x="${560-i*8}" y="${640+i*18}" width="${120+i*16}" height="4" fill="#ffe7b0" opacity="${(.7-i*.04).toFixed(2)}"/>`;
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f6c98e"/><stop offset=".55" stop-color="#f2a26a"/><stop offset=".72" stop-color="#e98a5a"/></linearGradient><linearGradient id="${wv}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#e3a27a"/><stop offset="1" stop-color="#d08a62"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/><circle cx="620" cy="610" r="80" fill="#fff0c8"/><rect y="630" width="${W}" height="270" fill="url(#${wv})"/>${refl}<path d="M0 650Q200 600 420 640L560 660H0Z" fill="#2b1a10"/>${palm(110,660,210,30)}${palm(230,660,170,-25)}${palm(880,640,200,25)}<path d="M760 640Q860 610 960 645H760Z" fill="#2b1a10"/>`)},box:[300,90,820,440]},
 {n:"Moonlit sea",tag:"pick",ar:["وهو معكم","أينما كنتم"],en:"And He is with you wherever you are",ref:"Al-Hadid 57:4",f:"NK",ink:"#eef2f6",pt:"#f2f6f8",gold:"#bfe0f2",
  bg(){const g=U("g"),h=U("h"),r=rnd(6);let sh="";for(let i=0;i<40;i++){const y=560+i*9;const wd=40+r()*120*(1+i/20);sh+=`<rect x="${(1150-wd/2+r()*30-15).toFixed(0)}" y="${y}" width="${wd.toFixed(0)}" height="2.5" fill="#fff" opacity="${(.55-i*.011).toFixed(2)}"/>`}
   let st="";for(let i=0;i<80;i++)st+=`<circle cx="${(r()*W).toFixed(0)}" cy="${(r()*520).toFixed(0)}" r="${(r()*1.2+.3).toFixed(2)}" fill="#fff" opacity="${(r()*.6+.2).toFixed(2)}"/>`;
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#03101f"/><stop offset=".62" stop-color="#123456"/><stop offset=".63" stop-color="#0a2238"/><stop offset="1" stop-color="#020a14"/></linearGradient><radialGradient id="${h}"><stop offset="0" stop-color="#fff" stop-opacity=".35"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/>${st}<circle cx="1150" cy="300" r="170" fill="url(#${h})"/><circle cx="1150" cy="300" r="62" fill="#f4f1e6"/>${sh}`)},box:[150,140,842,520]},
 {n:"Papercut mosque",tag:"pick",ar:["والله يحب","المحسنين"],en:"And Allah loves the doers of good",ref:"Al 'Imran 3:134",f:"RK",ink:"#24392a",pt:"#1c2a1f",gold:"#5d6d2a",
  bg(){const sh=U("s");const layer=(y,c,amp,seed)=>{const q=rnd(seed);let d=`M0 ${H}V${y}`;for(let x=0;x<=W;x+=100)d+=` Q${x+50} ${(y-amp*q()).toFixed(0)} ${x+100} ${y}`;return `<path d="${d}V${H}Z" fill="${c}" filter="url(#${sh})"/>`};
   return svg(`<filter id="${sh}" x="-5%" y="-20%" width="110%" height="140%"><feDropShadow dx="0" dy="-6" stdDeviation="8" flood-color="#1c2a1f" flood-opacity=".35"/></filter>`,
   `<rect width="${W}" height="${H}" fill="#eef0e4"/><circle cx="1100" cy="210" r="70" fill="#f6e6b8"/>${layer(640,"#cfd8bf",60,1)}<g filter="url(#${sh})">${bengal(520,696,.82,"#9caf86","#6f845e","#8a9c78")}</g>${layer(740,"#8fa27a",50,4)}${layer(820,"#5f7652",40,8)}`)},box:[150,110,842,500]},
 {n:"Terracotta niche",tag:"pick",ar:["لا تحزن","إن الله معنا"],en:"Do not grieve; indeed, Allah is with us",ref:"At-Tawbah 9:40",f:"CH",ink:"#fbeee0",pt:"#2e160c",gold:"#7a2e14",
  bg(){const n=U("n"),sh=U("s");let leaves="";const r=rnd(17);let px=760,py=110;for(let i=0;i<22;i++){px+=(r()-.5)*30;py+=26;leaves+=`<ellipse cx="${px.toFixed(0)}" cy="${py.toFixed(0)}" rx="16" ry="7" transform="rotate(${(r()*80-40).toFixed(0)} ${px.toFixed(0)} ${py.toFixed(0)})" fill="${r()>.5?"#4f7a3a":"#3a6230"}"/>`}
   return svg(grain(n,.28,.75)+`<linearGradient id="${sh}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#5a2412" stop-opacity=".55"/><stop offset=".25" stop-color="#5a2412" stop-opacity="0"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="#c9774f"/><path d="${archPath(226,120,540,700)}" fill="#a85a36"/><path d="${archPath(226,120,540,700)}" fill="url(#${sh})"/><rect x="200" y="820" width="592" height="26" fill="#e3a27a"/><rect x="200" y="846" width="592" height="10" fill="#7a3a20" opacity=".5"/><path d="M760 60V110" stroke="#5a3a20" stroke-width="2"/>${leaves}<path d="M300 820l14-60h52l14 60z" fill="#8a3e1e"/><path d="M340 760q-30 -60 -10 -110q20 50 10 110z" fill="#4f7a3a"/><path d="M340 760q30 -70 60 -90q-10 60 -60 90z" fill="#3a6230"/><rect width="${W}" height="${H}" filter="url(#${n})"/>`)},box:[290,300,700,700]},
 {n:"Book & candle",tag:"pick",ar:["ورتل القرآن","ترتيلا"],en:"And recite the Qur'an with measured recitation",ref:"Al-Muzzammil 73:4",f:"RK",ink:"#f6dfa6",pt:"#f6f2ea",gold:"#f0c26a",
  bg(){const g=U("g"),gl=U("l"),pg=U("p");let lines="";for(let i=0;i<7;i++){lines+=`<path d="M${330} ${700+i*14}q60 -6 130 0" stroke="#7a6a50" stroke-width="2" opacity=".5" fill="none"/><path d="M${510} ${700+i*14}q70 -6 130 0" stroke="#7a6a50" stroke-width="2" opacity=".5" fill="none"/>`}
   return svg(`<radialGradient id="${g}" cx=".55" cy=".6" r=".8"><stop offset="0" stop-color="#3a2614"/><stop offset="1" stop-color="#0d0805"/></radialGradient><radialGradient id="${gl}"><stop offset="0" stop-color="#ffcf7a" stop-opacity=".55"/><stop offset="1" stop-color="#ffcf7a" stop-opacity="0"/></radialGradient><linearGradient id="${pg}" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#e9dcc0"/><stop offset=".48" stop-color="#cbb894"/><stop offset=".52" stop-color="#cbb894"/><stop offset="1" stop-color="#e9dcc0"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/><circle cx="820" cy="560" r="300" fill="url(#${gl})"/><rect y="800" width="${W}" height="100" fill="#2a190c"/><path d="M290 800L330 690Q410 670 490 690Q570 670 650 690L690 800Q590 780 490 805Q390 780 290 800Z" fill="url(#${pg})"/><path d="M490 690V805" stroke="#9a875f" stroke-width="2"/>${lines}<path d="M290 800L270 815H710L690 800" fill="#5a1e14"/><rect x="800" y="610" width="40" height="190" fill="#f1e6cc"/><path d="M820 610q-12-24 0-50q12 26 0 50z" fill="#ffd27a"/><path d="M820 604q-5-12 0-24q5 12 0 24z" fill="#fff4d0"/><ellipse cx="820" cy="800" rx="46" ry="10" fill="#8a6a3a"/>`)},box:[150,120,842,560]},
 {n:"Ramadan night",sec:"occ",tag:"Ramadan",ar:["رمضان كريم"],en:"A generous Ramadan to you",ref:"Ramadan greeting",f:"RK",ink:"#f7e2a6",pt:"#f6f2ea",gold:"#f0c26a",s:1.45,
  bg(){const g=U("g"),gl=U("l"),mk=U("k"),r=rnd(31);let st="";for(let i=0;i<160;i++)st+=`<circle cx="${(r()*W).toFixed(0)}" cy="${(r()*520).toFixed(0)}" r="${(r()*1.4+.3).toFixed(2)}" fill="#fff" opacity="${(r()*.7+.2).toFixed(2)}"/>`;
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#04121f"/><stop offset=".7" stop-color="#0c3346"/><stop offset="1" stop-color="#15505e"/></linearGradient><radialGradient id="${gl}"><stop offset="0" stop-color="#ffd88a" stop-opacity=".6"/><stop offset="1" stop-color="#ffd88a" stop-opacity="0"/></radialGradient><mask id="${mk}"><circle cx="300" cy="190" r="120" fill="#fff"/><circle cx="350" cy="160" r="104" fill="#000"/></mask>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/>${st}<circle cx="300" cy="190" r="200" fill="url(#${gl})" opacity=".5"/><circle cx="300" cy="190" r="120" fill="#f7e2a6" mask="url(#${mk})"/>${fanous(258,296,1,gl)}${city(900,"#030c14",31,"#ffcf6a")}`)},box:[380,200,880,560]},
 {n:"Month of the Qur'an",sec:"occ",tag:"Ramadan",ar:["شهر رمضان","الذي أنزل فيه القرآن"],en:"The month of Ramadan, in which the Qur'an was revealed",ref:"Al-Baqarah 2:185",f:"RK",ink:"#f6dfa6",pt:"#f6f2ea",gold:"#f0c26a",s:.9,
  bg(){const g=U("g"),gl=U("l"),pg=U("p"),c=U("c"),mk=U("k"),r=rnd(7);let st="";for(let i=0;i<70;i++)st+=`<circle cx="${(330+r()*340).toFixed(0)}" cy="${(80+r()*360).toFixed(0)}" r="${(r()*1.3+.3).toFixed(2)}" fill="#fff" opacity="${(r()*.7+.2).toFixed(2)}"/>`;
   const a=archPath(320,60,360,470);
   return svg(`<radialGradient id="${g}" cx=".3" cy=".6" r=".8"><stop offset="0" stop-color="#3a2614"/><stop offset="1" stop-color="#0d0805"/></radialGradient><radialGradient id="${gl}"><stop offset="0" stop-color="#ffcf7a" stop-opacity=".55"/><stop offset="1" stop-color="#ffcf7a" stop-opacity="0"/></radialGradient><clipPath id="${c}"><path d="${a}"/></clipPath><mask id="${mk}"><circle cx="560" cy="170" r="46" fill="#fff"/><circle cx="580" cy="158" r="40" fill="#000"/></mask><linearGradient id="${pg}" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#e9dcc0"/><stop offset=".48" stop-color="#cbb894"/><stop offset=".52" stop-color="#cbb894"/><stop offset="1" stop-color="#e9dcc0"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/><g clip-path="url(#${c})"><rect width="${W}" height="${H}" fill="#0a2236"/>${st}<circle cx="560" cy="170" r="46" fill="#f4e3b0" mask="url(#${mk})"/>${city(530,"#05121d",4,"#ffcf6a")}</g><path d="${a}" fill="none" stroke="#5a3e24" stroke-width="22"/><rect x="290" y="530" width="420" height="26" fill="#5a3e24"/><circle cx="640" cy="520" r="120" fill="url(#${gl})"/>${fanous(640,430,.7,gl)}<path d="M360 530L384 470Q430 458 476 470Q522 458 568 470L592 530Q522 518 476 534Q430 518 360 530Z" fill="url(#${pg})"/>`)},box:[150,600,850,860]},
 {n:"Lantern street",sec:"occ",tag:"Ramadan",ar:["رمضان مبارك"],en:"Blessed Ramadan",ref:"Ramadan greeting",f:"RK",ink:"#3a1a0c",pt:"#2a140a",gold:"#a8401c",s:1.35,
  bg(){const g=U("g"),gl=U("l"),sh=U("s");
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f7d9a8"/><stop offset="1" stop-color="#f2b680"/></linearGradient><radialGradient id="${gl}"><stop offset="0" stop-color="#fff0c0" stop-opacity=".8"/><stop offset="1" stop-color="#fff0c0" stop-opacity="0"/></radialGradient><filter id="${sh}" x="-5%" y="-20%" width="110%" height="140%"><feDropShadow dx="0" dy="-6" stdDeviation="8" flood-color="#3a1a0c" flood-opacity=".35"/></filter>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/>${bunting(-20,60,940,120,"#a8401c","#1f6f5c","#e3b04b")}${bunting(-20,150,940,210,"#1f6f5c","#e3b04b","#a8401c")}<g filter="url(#${sh})">${city(760,"#d98a5a",12,"#ffe2a0")}</g><g filter="url(#${sh})">${city(830,"#b5653e",19,"#ffd27a")}</g><g filter="url(#${sh})">${city(900,"#7a3a20",27,"#ffcf6a")}</g>${fanous(150,200,.75,gl)}${fanous(820,250,.75,gl)}${veil("#f6d4a4")}`)},box:[180,300,842,560]},
 {n:"Laylat al-Qadr",sec:"occ",tag:"Last ten nights",ar:["ليلة القدر","خير من ألف شهر"],en:"The Night of Decree is better than a thousand months",ref:"Al-Qadr 97:3",f:"RK",ink:"#fbf1d0",pt:"#f6f2ea",gold:"#f3d58e",
  bg(){const g=U("g"),b=U("b"),r=rnd(97);let st="";for(let i=0;i<260;i++)st+=`<circle cx="${(r()*W).toFixed(0)}" cy="${(r()*700).toFixed(0)}" r="${(r()*1.6+.3).toFixed(2)}" fill="#fff" opacity="${(r()*.8+.2).toFixed(2)}"/>`;
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#020814"/><stop offset=".7" stop-color="#0b1d3c"/><stop offset="1" stop-color="#1a2f55"/></linearGradient><linearGradient id="${b}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff3c8" stop-opacity=".55"/><stop offset="1" stop-color="#fff3c8" stop-opacity="0"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/>${st}<polygon points="${CX-60},0 ${CX+60},0 ${CX+260},900 ${CX-260},900" fill="url(#${b})"/><polygon points="${CX-20},0 ${CX+20},0 ${CX+90},900 ${CX-90},900" fill="url(#${b})"/>${mamluk(CX+60,900,.8,"#01050c","#f3d58e","#1a2440")}`)},box:[150,90,842,430]},
 {n:"Eid morning",sec:"occ",tag:"Eid al-Fitr",ar:["عيد مبارك"],en:"Blessed Eid",ref:"Eid al-Fitr greeting",f:"RK",ink:"#5a2416",pt:"#3a1a10",gold:"#a8401c",s:1.4,
  bg(){const n=U("n"),c=U("c"),k=U("k"),sn=U("s"),gl=U("l"),br=U("b");
   const A="M200 900V500A310 310 0 1 1 800 500V900Z";
   const palmS=(x,y,h,col)=>{let f="";const tx=x+h*.12,ty=y-h;[[-90,10],[-70,40],[-30,55],[20,52],[60,38],[92,8]].forEach(([dx,dy])=>f+=`<path d="M${tx} ${ty}Q${tx+dx*.5} ${ty-28} ${tx+dx} ${ty+dy}" fill="none" stroke="${col}" stroke-width="7" stroke-linecap="round"/>`);return `<path d="M${x} ${y}Q${x+h*.05} ${y-h*.5} ${tx} ${ty}" fill="none" stroke="${col}" stroke-width="9"/>${f}`};
   let floor="";for(let r=0;r<5;r++)for(let i=0;i<16;i++){const y=772+r*28,x=170+i*44+(r%2)*22;floor+=`<polygon points="${star(x,y,10+r*1.5,5+r,8)}" fill="${(i+r)%3?"#1f6f5c":"#2a5a8a"}" opacity=".8"/>`}
   let pierce="";for(let i=0;i<5;i++)for(let j=0;j<3;j++)pierce+=`<circle cx="${486+j*14}" cy="${178+i*14+(j%2)*7}" r="2.4" fill="#fff6d0"/>`;
   return svg(grain(n,.2)+`<clipPath id="${c}"><path d="${A}"/></clipPath><linearGradient id="${k}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f4d8bc"/><stop offset=".55" stop-color="#fbe8d2"/><stop offset=".85" stop-color="#f7c9a2"/></linearGradient><radialGradient id="${sn}"><stop offset="0" stop-color="#fff4da"/><stop offset=".35" stop-color="#ffe3b4" stop-opacity=".8"/><stop offset="1" stop-color="#ffe3b4" stop-opacity="0"/></radialGradient><radialGradient id="${gl}"><stop offset="0" stop-color="#ffd98a" stop-opacity=".7"/><stop offset="1" stop-color="#ffd98a" stop-opacity="0"/></radialGradient><linearGradient id="${br}" x1="0" x2="1"><stop offset="0" stop-color="#8a5a1c"/><stop offset=".5" stop-color="#e2b25a"/><stop offset="1" stop-color="#7a4e18"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="#f1dfc3"/>
    <g clip-path="url(#${c})"><rect width="${W}" height="${H}" fill="url(#${k})"/><circle cx="600" cy="640" r="280" fill="url(#${sn})"/><circle cx="600" cy="660" r="58" fill="#fff3d8"/>
     <path d="M190 700V672h20v-10h14v10h20v-10h14v10h20v-10h14v10h20V700Z M560 700V680h16v-8h12v8h16V700Z" fill="#d8b3a0"/><path d="M190 760V700H820V760Z" fill="#d8b3a0"/>
     ${moroccan(600,760,.5,"#c39582","#5f8a6c","#f3d6bc")}${palmS(260,760,170,"#8a6656")}${palmS(760,760,140,"#8a6656")}
     <rect y="760" width="${W}" height="140" fill="#ecd5b4"/>${floor}<rect y="760" width="${W}" height="3" fill="#b08a5a"/></g>
    <path d="${A}" fill="none" stroke="#1f6f5c" stroke-width="44"/><path d="${A}" fill="none" stroke="#f6efe0" stroke-width="44" stroke-dasharray="9 23"/><path d="${A}" fill="none" stroke="#c0573a" stroke-width="44" stroke-dasharray="6 26" stroke-dashoffset="16"/>
    <path d="M222 900V500A288 288 0 1 1 778 500V900" fill="none" stroke="#c9a24e" stroke-width="3"/><path d="M178 900V500A332 332 0 1 1 822 500V900" fill="none" stroke="#c9a24e" stroke-width="3"/>
    <circle cx="500" cy="200" r="110" fill="url(#${gl})"/><path d="M500 88V146" stroke="#7a4e18" stroke-width="2" stroke-dasharray="5 3"/>
    <path d="M482 160Q500 136 518 160Z" fill="url(#${br})"/><path d="M476 160H524L532 176V244L520 262H480L468 244V176Z" fill="#ffe2a0" stroke="url(#${br})" stroke-width="5"/>${pierce}<path d="M476 160H524L532 176H468Z M480 262H520L506 278H494Z" fill="url(#${br})"/><path d="M500 278V292" stroke="#7a4e18" stroke-width="3"/>
    ${[[130,900],[870,900]].map(([x,y])=>`<path d="M${x-34} ${y-70}H${x+34}L${x+24} ${y}H${x-24}Z" fill="#b5653e"/><rect x="${x-38}" y="${y-78}" width="76" height="12" rx="3" fill="#9a5230"/>${[-40,-20,0,20,40].map((a,i)=>`<ellipse cx="${x+a*.9}" cy="${y-110-Math.abs(a)*.3-(i%2)*14}" rx="13" ry="38" transform="rotate(${a} ${x+a*.9} ${y-110})" fill="${i%2?"#4f7a3a":"#3f6a2e"}"/>`).join("")}`).join("")}
    <rect width="${W}" height="${H}" filter="url(#${n})"/>`)},box:[260,300,740,450]},

 {n:"Eid card",sec:"occ",tag:"Eid al-Fitr",ar:["تقبل الله","منا ومنكم"],en:"May Allah accept from us and from you",ref:"Eid greeting of the Companions",f:"RK",ink:"#f4dc9a",pt:"#1d2a22",gold:"#8a6420",s:1.05,
  bg(){const n=U("n"),sh=U("s"),sh2=U("t"),cl=U("c"),go=U("o"),pt=U("p"),r=rnd(61);
   const outer=archPath(250,120,500,600),inner=archPath(292,172,416,528);
   let conf="";for(let i=0;i<34;i++){const x=r()*960,y=r()*900;if(x>150&&x<850&&y>30&&y<880)continue;conf+=`<polygon points="${star(x,y,7+r()*6,3+r()*2,r()>.5?8:5)}" fill="#c9a24e" opacity="${(.5+r()*.5).toFixed(2)}" transform="rotate(${(r()*60).toFixed(0)} ${x.toFixed(0)} ${y.toFixed(0)})"/>`}
   let st="";for(let i=0;i<70;i++)st+=`<circle cx="${(300+r()*400).toFixed(0)}" cy="${(180+r()*480).toFixed(0)}" r="${(r()*1.3+.3).toFixed(2)}" fill="#fff" opacity="${(r()*.7+.2).toFixed(2)}"/>`;
   const charm=(x,len,kind)=>{const y=172+len;let o=`<line x1="${x}" y1="172" x2="${x}" y2="${y}" stroke="#d9b45a" stroke-width="1.4"/>`;
     if(kind==0)o+=`<polygon points="${star(x,y+20,22,9,8)}" fill="url(#${go})"/>`;
     else if(kind==1)o+=`<path d="M${x-4} ${y}A22 22 0 1 0 ${x+18} ${y+34}A17 17 0 1 1 ${x-4} ${y}Z" fill="url(#${go})"/>`;
     else o+=`<path d="M${x-12} ${y+8}H${x+12}L${x+16} ${y+16}V${y+42}L${x+8} ${y+52}H${x-8}L${x-16} ${y+42}V${y+16}Z" fill="#ffe2a0" stroke="url(#${go})" stroke-width="3"/><path d="M${x-6} ${y}H${x+6}L${x+12} ${y+8}H${x-12}Z" fill="url(#${go})"/>`;return o};
   return svg(grain(n,.18,.95)+`<filter id="${sh}" x="-10%" y="-10%" width="120%" height="130%"><feDropShadow dx="0" dy="16" stdDeviation="18" flood-color="#3a4a3a" flood-opacity=".3"/></filter><filter id="${sh2}" x="-10%" y="-10%" width="120%" height="120%"><feDropShadow dx="0" dy="5" stdDeviation="5" flood-color="#000" flood-opacity=".45"/></filter><clipPath id="${cl}"><path d="${inner}"/></clipPath><linearGradient id="${go}" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#f8e2a0"/><stop offset=".5" stop-color="#d9a84c"/><stop offset="1" stop-color="#a8742c"/></linearGradient>${girihTile(pt,"#c9a24e",.8,64)}`,
   `<rect width="${W}" height="${H}" fill="#dfe5d5"/>${conf}
    <rect x="180" y="50" width="640" height="810" rx="6" fill="#fbf6ea" filter="url(#${sh})"/><rect x="198" y="68" width="604" height="774" fill="none" stroke="url(#${go})" stroke-width="2.5"/><rect x="208" y="78" width="584" height="754" fill="none" stroke="#c9a24e" stroke-width="1" stroke-dasharray="2 5"/>
    <g clip-path="url(#${cl})"><rect width="${W}" height="${H}" fill="#0e2c44"/><rect width="${W}" height="${H}" fill="url(#${pt})" opacity=".14"/>${st}</g>
    <path d="${outer} ${inner}" fill="#1f5a4a" fill-rule="evenodd" filter="url(#${sh2})"/><path d="${inner}" fill="none" stroke="url(#${go})" stroke-width="2"/>
    <path d="M180 50H820V860H180Z ${outer}" fill="#fbf6ea" fill-rule="evenodd" filter="url(#${sh2})" opacity=".001"/><path d="${outer}" fill="none" stroke="#e9dcc0" stroke-width="3"/>
    ${charm(380,70,0)}${charm(500,30,1)}${charm(620,110,2)}
    <g transform="translate(500 790)"><circle r="30" fill="#a8322a"/><circle r="24" fill="none" stroke="#c94a3a" stroke-width="2"/><polygon points="${star(0,0,14,6,8)}" fill="#7a1e18"/></g>
    <path d="M300 760H460M540 760H700" stroke="#c9a24e" stroke-width="1.5"/><polygon points="${star(500,760,0,0,4)}"/>
    <rect width="${W}" height="${H}" filter="url(#${n})"/>`)},box:[318,400,682,680]},

 {n:"Day of Arafah",sec:"occ",tag:"Eid al-Adha",ar:["لبيك اللهم","لبيك"],en:"Here I am, O Allah, here I am",ref:"The Talbiyah of Hajj",f:"RK",ink:"#fff1dc",pt:"#f6f2ea",gold:"#ffd29a",s:1.2,
  bg(){const g=U("g"),sn=U("s"),hz=U("h"),gd=U("d"),r=rnd(40);
   const ridge=(base,amp,col,seed,step=40)=>{const q=rnd(seed);let d=`M0 ${H}V${base}`;for(let x=0;x<=W;x+=step)d+=` L${x} ${(base-amp*q()-amp*.4*Math.sin(x/220+seed)).toFixed(0)}`;return `<path d="${d}V${H}Z" fill="${col}"/>`};
   // Jabal al-Rahmah: a low, broad hill of angular granite boulders, lit by the setting sun on the right,
   // with the wide white staircase straight up its face and pilgrims in white all over it.
   const cx=480,top=604,base=800,hw=360,F=v=>v.toFixed(1),hc=U("c");
   const prof=x=>{const t=(x-cx)/hw;return top+(base-top)*Math.min(1,Math.pow(Math.abs(t),2.3)*1.02)+Math.sin(x*.09)*4+Math.sin(x*.031)*6};
   let outline=`M${cx-hw-30} ${base}`;for(let x=cx-hw-30;x<=cx+hw+30;x+=8)outline+=` L${x} ${F(Math.min(base,prof(x)))}`;outline+=` L${cx+hw+30} ${base}Z`;
   const rockPoly=(x,y,s,q)=>{let p=[];const n=6+Math.floor(q()*3);for(let i=0;i<n;i++){const a=i/n*6.283+q()*.4,rr=s*(.7+q()*.45);p.push([x+Math.cos(a)*rr,y+Math.sin(a)*rr*.75])}return p};
   const q=rnd(23);let rocks=[];for(let i=0;i<520;i++){const x=cx-hw-20+q()*(hw*2+40),y=top-10+q()*(base-top+10);if(y<prof(x)-6)continue;rocks.push([x,y,9+q()*24])}
   rocks.sort((a,b)=>a[1]-b[1]);
   let boulders="";rocks.forEach(([x,y,s])=>{const p=rockPoly(x,y,s,q),light=Math.max(0,Math.min(1,.45+(x-cx)/hw*.5)),depth=(y-top)/(base-top);
     const mix=(a,b,t)=>a.map((v,i)=>Math.round(v+(b[i]-v)*t)),hex=c=>"#"+c.map(v=>v.toString(16).padStart(2,"0")).join("");
     const baseC=mix([118,88,84],[78,56,58],depth*.6),lit=mix(baseC,[226,160,120],.35+light*.4),dark=mix(baseC,[40,26,30],.45);
     const pts=p.map(([a,b])=>`${F(a)},${F(b)}`).join(" ");
     const litPts=p.filter(([a,b])=>a>=x-s*.15||b<y-s*.2).map(([a,b])=>`${F(a)},${F(b)}`).join(" ");
     const darkPts=p.filter(([a,b])=>a<=x+s*.1&&b>=y-s*.1).map(([a,b])=>`${F(a)},${F(b)}`).join(" ");
     boulders+=`<polygon points="${pts}" fill="${hex(baseC)}" stroke="${hex(dark)}" stroke-width="1"/><polygon points="${F(x)},${F(y)} ${litPts}" fill="${hex(lit)}" opacity=".85"/><polygon points="${F(x)},${F(y)} ${darkPts}" fill="${hex(dark)}" opacity=".6"/>`;
     if(q()>.6)boulders+=`<path d="M${F(x-s*.3)} ${F(y-s*.1)}l${F(s*.3)} ${F(s*.25)}l${F(s*.2)} ${F(-s*.05)}" stroke="${hex(dark)}" stroke-width="1" fill="none"/>`});
   // the staircase: wide at the foot, narrowing towards the top, steps and handrails
   const sx0=cx-14,sx1=cx+26,sy0=base,sxt0=cx-2,sxt1=cx+9,syt=prof(cx)+6;let stairs=`<path d="M${sx0} ${sy0}L${sxt0} ${F(syt)}H${sxt1}L${sx1} ${sy0}Z" fill="#e6dccb"/><path d="M${(sx0+sx1)/2} ${sy0}L${(sxt0+sxt1)/2} ${F(syt)}L${sx1} ${sy0}Z" fill="#f6efe2"/>`;
   for(let i=1;i<34;i++){const t=i/34,y=sy0+(syt-sy0)*t,l=sx0+(sxt0-sx0)*t,r2=sx1+(sxt1-sx1)*t;stairs+=`<path d="M${F(l)} ${F(y)}H${F(r2)}" stroke="#bfb3a0" stroke-width="${F(1.4-t)}"/>`}
   stairs+=`<path d="M${sx0-3} ${sy0}L${sxt0-2} ${F(syt)}M${sx1+3} ${sy0}L${sxt1+2} ${F(syt)}" stroke="#d8cfc0" stroke-width="2"/>`;
   // pilgrims: crowded on the stairs, scattered over the rocks, thick at the foot of the hill
   let pil="";const man=(x,y,s)=>`<rect x="${F(x-1.6*s)}" y="${F(y-6.4*s)}" width="${F(3.2*s)}" height="${F(6.4*s)}" rx="${F(1.6*s)}" fill="#fffaf0"/>`;
   for(let i=0;i<70;i++){const t=q(),y=sy0+(syt-sy0)*t,l=sx0+(sxt0-sx0)*t,r2=sx1+(sxt1-sx1)*t;pil+=man(l+q()*(r2-l),y,1-t*.4)}
   for(let i=0;i<260;i++){const x=cx-hw+q()*hw*2,y=prof(x)+4+q()*(base-prof(x));if(y>base||y<prof(x)+3)continue;const near=(y-top)/(base-top);if(q()>.35+near*.6)continue;pil+=man(x,y,.75+near*.4)}
   for(let i=0;i<220;i++){const x=cx-hw-60+q()*(hw*2+120),y=base+2+q()*26;pil+=man(x,y,1.1+(y-base)/40)}
   // tent city far away in the haze: tiny white peaked tents in rows that fade into the distance
   let tents="";[[748,7,.35],[758,9,.45],[770,11,.55],[784,13,.7],[800,16,.85]].forEach(([y,w,o],ri)=>{for(let x=(ri%2)*w;x<900;x+=w*1.35){const fade=x>760?Math.max(0,(900-x)/140):1;if(x>cx-hw*.95&&x<cx+hw*.95&&y<base-6)continue;
     tents+=`<path d="M${(x-w/2).toFixed(1)} ${y}V${(y-w*.35).toFixed(1)}L${x.toFixed(1)} ${(y-w*.8).toFixed(1)}L${(x+w/2).toFixed(1)} ${(y-w*.35).toFixed(1)}V${y}Z" fill="#f4ece0" opacity="${(o*fade).toFixed(2)}"/><path d="M${x.toFixed(1)} ${(y-w*.8).toFixed(1)}L${(x+w/2).toFixed(1)} ${(y-w*.35).toFixed(1)}V${y}H${x.toFixed(1)}Z" fill="#c9b8a8" opacity="${(o*fade).toFixed(2)}"/>`}});
   // acacia trees in the foreground
   const acacia=(x,y,s)=>`<path d="M${x} ${y}V${y-40*s}M${x} ${y-24*s}L${x-22*s} ${y-46*s}M${x} ${y-30*s}L${x+26*s} ${y-50*s}" stroke="#1e1216" stroke-width="${4*s}" stroke-linecap="round"/><ellipse cx="${x}" cy="${y-52*s}" rx="${54*s}" ry="${11*s}" fill="#1e1216"/><ellipse cx="${x-18*s}" cy="${y-58*s}" rx="${28*s}" ry="${7*s}" fill="#1e1216"/>`;
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1f2c58"/><stop offset=".5" stop-color="#7a4466"/><stop offset=".78" stop-color="#d27a5c"/><stop offset="1" stop-color="#e9a070"/></linearGradient><radialGradient id="${sn}"><stop offset="0" stop-color="#fff2cc"/><stop offset=".25" stop-color="#ffd59a" stop-opacity=".85"/><stop offset="1" stop-color="#ffd59a" stop-opacity="0"/></radialGradient><linearGradient id="${hz}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#e9a070" stop-opacity="0"/><stop offset="1" stop-color="#f0b080" stop-opacity=".55"/></linearGradient><linearGradient id="${gd}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#6a4038"/><stop offset="1" stop-color="#2a1a1e"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/><circle cx="720" cy="600" r="300" fill="url(#${sn})"/><circle cx="720" cy="610" r="40" fill="#fff4d6"/>
    ${ridge(680,70,"#8a5466",3)}${ridge(730,45,"#74485a",8,30)}<rect y="600" width="${W}" height="200" fill="url(#${hz})"/>
    <rect y="745" width="${W}" height="155" fill="url(#${gd})"/>${tents}
    <ellipse cx="${cx}" cy="${base}" rx="${hw*1.12}" ry="14" fill="#22161a" opacity=".5"/><clipPath id="${hc}"><path d="${outline}"/></clipPath><path d="${outline}" fill="#5a4040"/><g clip-path="url(#${hc})">${boulders}</g>${stairs}${pil}
    <path d="M0 830Q500 812 1000 838T1600 830V900H0Z" fill="#24161a"/>${acacia(90,840,1.1)}${acacia(860,846,.9)}`)},box:[150,80,842,370]},

 {n:"Eid prayer at the eidgah",sec:"occ",tag:"Eid al-Adha",ar:["عيد أضحى","مبارك"],en:"Blessed Eid al-Adha",ref:"Eid al-Adha greeting",f:"RK",ink:"#24392a",pt:"#1c2a1f",gold:"#7a5a1c",s:1.2,
  bg(){const g=U("g"),sn=U("s"),gr=U("r"),r=rnd(77);
   const tree=(x,y,s)=>{let t=`<path d="M${x-8*s} ${y}L${x-4*s} ${y-90*s}H${x+4*s}L${x+8*s} ${y}Z" fill="#4a3a2a"/>`;[[0,-150,70],[-55,-120,52],[55,-118,55],[-30,-185,48],[35,-180,46],[-80,-90,36],[80,-92,38]].forEach(([dx,dy,rr],i)=>t+=`<circle cx="${x+dx*s}" cy="${y+dy*s}" r="${rr*s}" fill="${i%2?"#4f6b40":"#5c7a4a"}"/>`);return t};
   // long white sheets laid in straight rows across the field for the Eid prayer, as at eidgahs in Bangladesh
   let rugs="";[[660,8,230],[690,12,280],[728,17,330],[776,23,380],[836,31,420]].forEach(([y,h,hw],row)=>{const F=v=>v.toFixed(1),x0=CX-hw,x1=CX+hw,k=h/20;
     rugs+=`<path d="M${F(x0+3)} ${F(y+h+3*k)}H${F(x1+3)}" stroke="#4f6a3a" stroke-width="${F(3*k)}" opacity=".35"/>`
      +`<path d="M${F(x0)} ${F(y)}H${F(x1)}L${F(x1+hw*.01)} ${F(y+h)}H${F(x0-hw*.01)}Z" fill="#f8f5ec"/>`
      +`<path d="M${F(x0)} ${F(y)}H${F(x1)}" stroke="#ffffff" stroke-width="${F(1.2*k)}"/><path d="M${F(x0-hw*.01)} ${F(y+h)}H${F(x1+hw*.01)}" stroke="#d9d3c4" stroke-width="${F(1.4*k)}"/>`;
     for(let x=x0+40*k+r()*30;x<x1-20;x+=55*k+r()*70*k){const w=4*k+r()*6*k;rugs+=`<path d="M${F(x)} ${F(y+1)}q${F(w*.4)} ${F(h*.5)} ${F(w*.1)} ${F(h-1)}" stroke="#e2dccd" stroke-width="${F(1.3*k)}" fill="none"/>`}});
   let gar="";for(let i=0;i<=46;i++){const t=i/46,x=CX-330+660*t,y=612-118+Math.sin(t*Math.PI*6)*-10+20;gar+=`<circle cx="${x.toFixed(1)}" cy="${(y+Math.abs(Math.sin(t*Math.PI*6))*14).toFixed(1)}" r="4.2" fill="${i%2?"#f2a33a":"#e9732a"}"/>`}
   let birds="";[[300,200],[340,180],[372,206],[700,150],[730,170]].forEach(([x,y])=>birds+=`<path d="M${x-10} ${y}q5 -6 10 0q5 -6 10 0" fill="none" stroke="#4a4038" stroke-width="2"/>`);
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#cfe0ea"/><stop offset=".45" stop-color="#f4e6cf"/><stop offset=".68" stop-color="#f8cfa2"/></linearGradient><radialGradient id="${sn}"><stop offset="0" stop-color="#fff6dc"/><stop offset=".3" stop-color="#ffe6b8" stop-opacity=".8"/><stop offset="1" stop-color="#ffe6b8" stop-opacity="0"/></radialGradient><linearGradient id="${gr}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#a9bd7e"/><stop offset="1" stop-color="#76925a"/></linearGradient>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/><circle cx="${CX}" cy="430" r="300" fill="url(#${sn})"/>${birds}
    ${tree(100,640,1.1)}${tree(900,640,1)}<rect y="636" width="${W}" height="264" fill="url(#${gr})"/>
    ${eidgah(CX,640,1.35,"#c98a5a","#5a2418")}<path d="M${CX-446} 640H${CX+446}" stroke="#8a5432" stroke-width="5"/>${[-1,1].map(k=>`<path d="M${CX+k*151} ${640-270} v-30" stroke="#e9d6a0" stroke-width="3"/><circle cx="${CX+k*151}" cy="${640-306}" r="5" fill="#e9d6a0"/>`).join("")}
    ${rugs}${veil("#eef1e2")}`)},box:[150,60,842,300]},
 {n:"Takbir of Eid",sec:"occ",tag:"Eid al-Fitr & al-Adha",ar:["الله أكبر الله أكبر","لا إله إلا الله","الله أكبر ولله الحمد"],en:"Allah is the Greatest, there is no god but Allah, and to Allah belongs all praise",ref:"Takbir of the Eid days",f:"RK",ink:"#f6e3a8",pt:"#f6f2ea",gold:"#e2c27a",s:.8,
  bg(){const g=U("g"),gl=U("l"),pc=U("c"),r=rnd(55);let st="";for(let i=0;i<150;i++)st+=`<circle cx="${(r()*W).toFixed(0)}" cy="${(r()*600).toFixed(0)}" r="${(r()*1.3+.3).toFixed(2)}" fill="#fff" opacity="${(r()*.6+.2).toFixed(2)}"/>`;
   const burst=(x,y,R,c,n)=>{let s=`<circle cx="${x}" cy="${y}" r="${R*.5}" fill="${c}" opacity=".08"/>`;for(let k=0;k<n;k++){const a=k/n*6.283+r()*.1,ex=x+Math.cos(a)*R,ey=y+Math.sin(a)*R+R*.18;
     s+=`<path d="M${(x+Math.cos(a)*R*.15).toFixed(0)} ${(y+Math.sin(a)*R*.15).toFixed(0)}Q${(x+Math.cos(a)*R*.7).toFixed(0)} ${(y+Math.sin(a)*R*.7).toFixed(0)} ${ex.toFixed(0)} ${ey.toFixed(0)}" stroke="${c}" stroke-width="1.6" fill="none" opacity=".55" stroke-linecap="round"/>`;
     [.45,.62,.78,.92,1].forEach((f,j)=>s+=`<circle cx="${(x+(ex-x)*f).toFixed(1)}" cy="${(y+(ey-y)*f).toFixed(1)}" r="${(1+j*.5).toFixed(1)}" fill="${c}" opacity="${(.4+j*.12).toFixed(2)}"/>`)}
     return s+`<circle cx="${x}" cy="${y}" r="4" fill="#fff"/>`};
   const s=.7,cx=CX,b=800;const lights=(x1,y1,x2,y2,sag)=>{let o="";const mx=(x1+x2)/2,my=Math.max(y1,y2)+sag;for(let i=0;i<=40;i++){const t=i/40,px=(1-t)*(1-t)*x1+2*(1-t)*t*mx+t*t*x2,py=(1-t)*(1-t)*y1+2*(1-t)*t*my+t*t*y2;o+=`<circle cx="${px.toFixed(1)}" cy="${py.toFixed(1)}" r="2.4" fill="${["#ffd98a","#ffe9b8","#ffc46a"][i%3]}"/>`}return o};
   const m=mughal(cx,b,s,"#3a1a18","#efe4cf","#ffcf6a");
   return svg(`<linearGradient id="${g}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#050c22"/><stop offset=".65" stop-color="#14284f"/><stop offset=".89" stop-color="#26406e"/></linearGradient><radialGradient id="${gl}"><stop offset="0" stop-color="#ffdf9e" stop-opacity=".35"/><stop offset="1" stop-color="#ffdf9e" stop-opacity="0"/></radialGradient><clipPath id="${pc}"><rect y="${b}" width="${W}" height="${H-b}"/></clipPath>`,
   `<rect width="${W}" height="${H}" fill="url(#${g})"/>${st}${burst(190,150,110,"#f3d58e",36)}${burst(820,120,95,"#9fd2e8",32)}${burst(90,380,60,"#f2a2b0",24)}${burst(900,360,55,"#f3d58e",22)}
    <circle cx="${cx}" cy="${b-330*s}" r="300" fill="url(#${gl})"/>${m}${lights(cx-395*s,b-470*s,cx,b-524*s,40)}${lights(cx,b-524*s,cx+395*s,b-470*s,40)}
    <rect y="${b}" width="${W}" height="${H-b}" fill="#081428"/><g clip-path="url(#${pc})"><g transform="translate(0 ${2*b}) scale(1 -1)" opacity=".22">${m}</g>${[...Array(8)].map((_,i)=>`<rect x="${cx-200+r()*400}" y="${b+10+i*11}" width="${60+r()*120}" height="1.5" fill="#ffe2a0" opacity=".25"/>`).join("")}</g>`)},box:[150,120,842,400]},

];
