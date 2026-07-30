import { create } from "zustand";

export type Screen = "revisar" | "deck" | "estatisticas" | "configuracoes";

interface MainState {
  screen: Screen;
  setScreen: (screen: Screen) => void;
}

/** Navegacao da janela principal. */
export const useMainStore = create<MainState>((set) => ({
  screen: "revisar",
  setScreen: (screen) => set({ screen }),
}));
