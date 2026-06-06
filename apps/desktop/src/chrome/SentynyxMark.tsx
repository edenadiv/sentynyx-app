export function SentynyxMark({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" style={{ display: "block" }}>
      <defs>
        <radialGradient id="lg-core" cx="50%" cy="50%" r="50%">
          <stop offset="0%" stopColor="#f2ff2b" stopOpacity="1" />
          <stop offset="60%" stopColor="#f2ff2b" stopOpacity="0.2" />
          <stop offset="100%" stopColor="#f2ff2b" stopOpacity="0" />
        </radialGradient>
      </defs>
      <circle cx="32" cy="32" r="30" fill="url(#lg-core)" />
      <circle cx="32" cy="32" r="10" fill="#05060a" stroke="#f2ff2b" strokeWidth="1.2" />
      <circle cx="32" cy="32" r="18" fill="none" stroke="#f2ff2b" strokeWidth="0.6" strokeDasharray="2 3" opacity="0.7">
        <animateTransform attributeName="transform" type="rotate" from="0 32 32" to="360 32 32" dur="18s" repeatCount="indefinite" />
      </circle>
      <circle cx="32" cy="32" r="26" fill="none" stroke="#f2ff2b" strokeWidth="0.4" strokeDasharray="1 4" opacity="0.4">
        <animateTransform attributeName="transform" type="rotate" from="360 32 32" to="0 32 32" dur="40s" repeatCount="indefinite" />
      </circle>
      <circle cx="32" cy="32" r="2.2" fill="#f2ff2b" />
    </svg>
  );
}
