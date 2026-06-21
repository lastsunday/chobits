import type { Frame } from "@/data/frame";
import type { PaginatedData, Round } from "@/data/round";
import type { RoundData } from "@/data/round-data";
import { getJson, instance } from "./http";

export async function listRounds(params?: {
  user_id?: string;
  page?: number;
  page_size?: number;
}): Promise<PaginatedData<Round>> {
  return getJson("/api/record/rounds", params as Record<string, unknown>);
}

export async function getRound(id: string): Promise<Round> {
  return getJson(`/api/record/rounds/${id}`);
}

export async function listRoundData(roundId: string): Promise<RoundData[]> {
  return getJson(`/api/record/rounds/${roundId}/data`);
}

export async function getAudioBlob(roundId: string, dataId: string): Promise<Blob> {
  const resp = await instance.get(
    `/api/record/rounds/${roundId}/data/${dataId}/blob`,
    { responseType: "blob" },
  );
  return resp.data;
}

export async function listFrames(
  roundId: string,
  params?: { page?: number; page_size?: number },
): Promise<PaginatedData<Frame>> {
  return getJson(
    `/api/record/rounds/${roundId}/frames`,
    params as Record<string, unknown>,
  );
}
