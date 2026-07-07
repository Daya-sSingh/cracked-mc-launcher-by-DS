import {
  useQuery,
  useMutation,
  useQueryClient,
  type UseQueryResult,
  type UseMutationResult,
} from "@tanstack/react-query";
import * as ipc from "@/lib/tauri";
import { useInstanceStore } from "@/state/instances";
import type {
  CreateInstanceRequest,
  FabricLoaderForGame,
  Instance,
  InstanceUpdate,
  ModFileInfo,
  VersionManifest,
} from "@/types";

// ─── Query keys — centralised so refetch/invalidation never uses a magic string ──

export const QUERY_KEYS = {
  instances: ["instances"] as const,
  instance: (id: string) => ["instances", id] as const,
  instanceMods: (id: string) => ["instance_mods", id] as const,
  versionManifest: ["version_manifest"] as const,
  fabricLoaderVersions: (gameVersion: string) => ["fabric_loader_versions", gameVersion] as const,
};

// ─── Instance queries ─────────────────────────────────────────────────────────

export function useInstances(): UseQueryResult<Instance[]> {
  const { sortOrder, setInstances, setLoading, setError } = useInstanceStore();

  return useQuery({
    queryKey: [...QUERY_KEYS.instances, sortOrder],
    queryFn: async () => {
      setLoading(true);
      try {
        const instances = await ipc.listInstances(sortOrder);
        setInstances(instances);
        return instances;
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        throw err;
      }
    },
    staleTime: 30_000,
    refetchOnWindowFocus: false,
  });
}

export function useInstance(id: string): UseQueryResult<Instance> {
  return useQuery({
    queryKey: QUERY_KEYS.instance(id),
    queryFn: () => ipc.getInstance(id),
    staleTime: 30_000,
    enabled: !!id,
  });
}

// ─── Instance mutations ───────────────────────────────────────────────────────

export function useCreateInstance(): UseMutationResult<Instance, Error, CreateInstanceRequest> {
  const qc = useQueryClient();
  const { upsertInstance } = useInstanceStore();

  return useMutation({
    mutationFn: ipc.createInstance,
    onSuccess: (instance) => {
      upsertInstance(instance);
      qc.invalidateQueries({ queryKey: QUERY_KEYS.instances });
    },
  });
}

export function useUpdateInstance(): UseMutationResult<
  Instance,
  Error,
  { id: string; update: InstanceUpdate }
> {
  const qc = useQueryClient();
  const { upsertInstance } = useInstanceStore();

  return useMutation({
    mutationFn: ({ id, update }) => ipc.updateInstance(id, update),
    onSuccess: (instance) => {
      upsertInstance(instance);
      qc.invalidateQueries({ queryKey: QUERY_KEYS.instance(instance.id) });
    },
  });
}

export function useDeleteInstance(): UseMutationResult<void, Error, string> {
  const qc = useQueryClient();
  const { removeInstance } = useInstanceStore();

  return useMutation({
    mutationFn: ipc.deleteInstance,
    onSuccess: (_, id) => {
      removeInstance(id);
      qc.invalidateQueries({ queryKey: QUERY_KEYS.instances });
    },
  });
}

export function useToggleFavorite(): UseMutationResult<
  Instance,
  Error,
  { id: string; favorite: boolean }
> {
  const { upsertInstance } = useInstanceStore();

  return useMutation({
    mutationFn: ({ id, favorite }) => ipc.updateInstance(id, { favorite }),
    onSuccess: (instance) => upsertInstance(instance),
  });
}

// ─── Version manifest ─────────────────────────────────────────────────────────

export function useVersionManifest(enabled = true): UseQueryResult<VersionManifest> {
  return useQuery({
    queryKey: QUERY_KEYS.versionManifest,
    queryFn: () => ipc.getVersionManifest(),
    staleTime: 60 * 60 * 1000,  // 1 hour — matches the backend cache TTL
    refetchOnWindowFocus: false,
    enabled,
  });
}

// ─── Fabric loader versions ───────────────────────────────────────────────────

/**
 * Every Fabric Loader build compatible with `gameVersion`. Only fetches
 * when `enabled` (the create-instance modal passes `loader === "fabric"`)
 * — there's no point calling Fabric's API while the user hasn't chosen
 * that loader yet.
 */
export function useFabricLoaderVersions(
  gameVersion: string,
  enabled: boolean,
): UseQueryResult<FabricLoaderForGame[]> {
  return useQuery({
    queryKey: QUERY_KEYS.fabricLoaderVersions(gameVersion),
    queryFn: () => ipc.getFabricLoaderVersions(gameVersion),
    staleTime: 5 * 60 * 1000,
    refetchOnWindowFocus: false,
    enabled: enabled && gameVersion.trim().length > 0,
  });
}

// ─── Instance mods (read-only listing, until the full mod browser lands) ─────

/** Every `.jar` currently in an instance's mods folder. */
export function useInstanceMods(instanceId: string, enabled = true): UseQueryResult<ModFileInfo[]> {
  return useQuery({
    queryKey: QUERY_KEYS.instanceMods(instanceId),
    queryFn: () => ipc.listInstanceMods(instanceId),
    staleTime: 10_000,
    refetchOnWindowFocus: false,
    enabled: enabled && !!instanceId,
  });
}

/** Imports dropped/browsed `.jar` files into an instance's mods folder, then refreshes the mod list. */
export function useImportModFiles(): UseMutationResult<
  number,
  Error,
  { instanceId: string; filePaths: string[] }
> {
  const qc = useQueryClient();

  return useMutation({
    mutationFn: ({ instanceId, filePaths }) => ipc.importModFiles(instanceId, filePaths),
    onSuccess: (_, { instanceId }) => {
      qc.invalidateQueries({ queryKey: QUERY_KEYS.instanceMods(instanceId) });
    },
  });
}
