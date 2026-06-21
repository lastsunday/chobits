export interface Frame {
  id: number;
  round_id: string;
  seq: number;
  dir: string;
  kind: string;
  detail: string | null;
  create_datetime: string | null;
  update_datetime: string | null;
}
