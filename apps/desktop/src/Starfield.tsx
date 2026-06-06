import { useEffect } from "react";

/** Canvas starfield + shooting stars. Mounts once on App, paints into #stars. */
export function Starfield() {
  useEffect(() => {
    const c = document.getElementById("stars") as HTMLCanvasElement | null;
    if (!c) return;
    const ctx = c.getContext("2d")!;
    let w = 0, h = 0;
    type Star = { x:number; y:number; z:number; r:number; tw:number; sp:number };
    type Shoot = { x:number; y:number; vx:number; vy:number; life:number };
    let stars: Star[] = [];
    const shoot: Shoot[] = [];
    const resize = () => { w = c.width = innerWidth * devicePixelRatio; h = c.height = innerHeight * devicePixelRatio; };
    resize();
    const init = () => {
      stars = [];
      const N = Math.floor((innerWidth * innerHeight) / 3500);
      for (let i = 0; i < N; i++) {
        stars.push({ x: Math.random()*w, y: Math.random()*h, z: Math.random()*1+0.2,
          r: Math.random()*1.2+0.2, tw: Math.random()*Math.PI*2, sp: 0.002 + Math.random()*0.01 });
      }
    };
    init();
    addEventListener("resize", () => { resize(); init(); });

    let raf = 0;
    const tick = () => {
      ctx.clearRect(0, 0, w, h);
      for (const s of stars) {
        s.tw += s.sp;
        const a = 0.35 + 0.65 * ((Math.sin(s.tw) + 1) / 2) * s.z;
        ctx.beginPath();
        ctx.fillStyle = `rgba(255,255,255,${a})`;
        ctx.arc(s.x, s.y, s.r * devicePixelRatio, 0, Math.PI * 2);
        ctx.fill();
        s.x += 0.02 * s.z * devicePixelRatio;
        if (s.x > w) s.x = 0;
      }
      if (Math.random() < 0.005) {
        shoot.push({ x: Math.random()*w*0.8, y: Math.random()*h*0.4, vx: 8+Math.random()*6, vy: 2+Math.random()*2, life: 1 });
      }
      for (let i = shoot.length - 1; i >= 0; i--) {
        const s = shoot[i];
        ctx.strokeStyle = `rgba(242,255,43,${s.life})`;
        ctx.lineWidth = 1.2 * devicePixelRatio;
        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(s.x - s.vx * 10, s.y - s.vy * 10);
        ctx.stroke();
        s.x += s.vx * devicePixelRatio;
        s.y += s.vy * devicePixelRatio;
        s.life -= 0.015;
        if (s.life <= 0) shoot.splice(i, 1);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);
  return null;
}
