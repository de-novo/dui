export function Probe({ dynamic }: { dynamic: string }) {
  return (
    <main>
      <div className="track move">CSS transition</div>
      <div className="track pulse">CSS keyframes</div>
      <div className="track animate-[slide_1s_linear_both]">Tailwind utility</div>
      <div className={dynamic}>Runtime class</div>
    </main>
  );
}
