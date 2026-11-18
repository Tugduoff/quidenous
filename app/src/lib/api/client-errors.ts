import { api } from '@/lib/api/client';

export type ClientErrorReport = {
  message: string;
  stack?: string;
  componentStack?: string;
  platform: string;
  version: string;
  route?: string;
};

export const postClientError = (report: ClientErrorReport) => api('/client-errors', { method: 'POST', json: report });
