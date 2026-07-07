import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type { Instance, InstanceSort } from "@/types";

interface InstanceStoreState {
  instances: Instance[];
  selectedInstanceId: string | null;
  sortOrder: InstanceSort;
  searchQuery: string;
  isLoading: boolean;
  error: string | null;

  // Actions
  setInstances: (instances: Instance[]) => void;
  upsertInstance: (instance: Instance) => void;
  removeInstance: (id: string) => void;
  selectInstance: (id: string | null) => void;
  setSortOrder: (order: InstanceSort) => void;
  setSearchQuery: (query: string) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

export const useInstanceStore = create<InstanceStoreState>()(
  subscribeWithSelector((set) => ({
    instances: [],
    selectedInstanceId: null,
    sortOrder: "recently_played",
    searchQuery: "",
    isLoading: false,
    error: null,

    setInstances: (instances) => set({ instances, isLoading: false, error: null }),

    upsertInstance: (instance) =>
      set((state) => {
        const idx = state.instances.findIndex((i) => i.id === instance.id);
        if (idx === -1) {
          return { instances: [instance, ...state.instances] };
        }
        const next = [...state.instances];
        next[idx] = instance;
        return { instances: next };
      }),

    removeInstance: (id) =>
      set((state) => ({
        instances: state.instances.filter((i) => i.id !== id),
        selectedInstanceId:
          state.selectedInstanceId === id ? null : state.selectedInstanceId,
      })),

    selectInstance: (id) => set({ selectedInstanceId: id }),
    setSortOrder: (order) => set({ sortOrder: order }),
    setSearchQuery: (query) => set({ searchQuery: query }),
    setLoading: (loading) => set({ isLoading: loading }),
    setError: (error) => set({ error }),
  })),
);

// ─── Derived selectors ────────────────────────────────────────────────────────

/** Returns the filtered + sorted list the UI actually renders. */
export function selectFilteredInstances(state: InstanceStoreState): Instance[] {
  const q = state.searchQuery.trim().toLowerCase();
  let list = q
    ? state.instances.filter(
        (i) =>
          i.name.toLowerCase().includes(q) ||
          i.minecraft_version.toLowerCase().includes(q) ||
          (i.loader !== "vanilla" && i.loader.toLowerCase().includes(q)),
      )
    : [...state.instances];

  // The list is already sorted on the backend, but re-sort client-side to
  // reflect optimistic updates (e.g. a renamed instance moving in the list
  // before the next server round-trip).
  switch (state.sortOrder) {
    case "name_ascending":
      list.sort((a, b) => a.name.localeCompare(b.name));
      break;
    case "name_descending":
      list.sort((a, b) => b.name.localeCompare(a.name));
      break;
    case "favorites_first":
      list.sort((a, b) => (b.favorite ? 1 : 0) - (a.favorite ? 1 : 0));
      break;
    case "recently_played":
    default:
      list.sort((a, b) => {
        if (!a.last_played_at && !b.last_played_at) return 0;
        if (!a.last_played_at) return 1;
        if (!b.last_played_at) return -1;
        return b.last_played_at.localeCompare(a.last_played_at);
      });
      break;
  }

  return list;
}

export function selectSelectedInstance(state: InstanceStoreState): Instance | null {
  if (!state.selectedInstanceId) return null;
  return state.instances.find((i) => i.id === state.selectedInstanceId) ?? null;
}
