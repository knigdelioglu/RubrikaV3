import { blockingReasonLabels } from '../../utils/labels';

export function BlockingReasons({ reasons }: { reasons: string[] }) {
  if (!reasons || reasons.length === 0) return null;
  return (
    <div style={{ marginTop: '1rem' }}>
      <h4>Engeller:</h4>
      <ul>
        {reasons.map((r, i) => (
          <li key={i} style={{ color: '#b91c1c' }}>{blockingReasonLabels[r] || r}</li>
        ))}
      </ul>
    </div>
  );
}
