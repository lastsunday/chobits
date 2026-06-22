import type { TurnStep } from '@/data/session';
import { Badge } from '@mantine/core';

const stepLabel: Record<string, string> = {
  input_audio: '语音输入',
  asr: '语音识别',
  text: '文本输入',
  llm: '大模型',
  tts: '语音合成',
};

const stepColor: Record<string, string> = {
  input_audio: 'gray',
  asr: 'gray',
  text: 'gray',
  llm: 'gray',
  tts: 'gray',
};

function truncate(text: string, maxLen: number): string {
  if (text.length <= maxLen) return `"${text}"`;
  return `"${text.slice(0, maxLen)}..." (${text.length}字)`;
}

export function StepBadge({ step }: { step: TurnStep }) {
  const label = stepLabel[step.step] ?? step.step;
  const color = stepColor[step.step] ?? 'gray';

  if (!step.has_data) {
    return (
      <Badge size="md" variant="light" color="gray" style={{ opacity: 0.5, textTransform: 'none' }}>
        {label} ✗
      </Badge>
    );
  }

  const dataPart = step.audio_duration_ms != null
    ? `✓ ${(step.audio_duration_ms / 1000).toFixed(1)}s`
    : step.text
      ? truncate(step.text, 10)
      : '✓';

  const procPart = step.duration_ms != null && step.step !== 'input_audio'
    ? `⏱${(step.duration_ms / 1000).toFixed(step.duration_ms < 1000 ? 1 : 0)}s`
    : null;

  return (
    <Badge size="md" variant="light" color={color} style={{ textTransform: 'none' }}>
      {label} {dataPart}{procPart != null ? `｜${procPart}` : ''}
    </Badge>
  );
}
