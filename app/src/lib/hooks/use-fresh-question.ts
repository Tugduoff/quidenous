import { useEffect, useState } from 'react';

let lastQuestion: number | null = null;

export function useFreshQuestion(roundId: number, result: boolean) {
  const [fresh] = useState(() => !result && roundId !== lastQuestion);
  useEffect(() => {
    if (!result) lastQuestion = roundId;
  }, [result, roundId]);
  return fresh;
}
