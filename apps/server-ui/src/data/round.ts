export interface Round {
  id: string;
  user_id: string | null;
  client_info: Record<string, unknown> | null;
  create_datetime: string | null;
  update_datetime: string | null;
}

export interface PaginatedData<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
}
