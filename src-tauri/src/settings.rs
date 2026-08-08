//! Configuracoes do usuario persistidas entre sessoes.
//!
//! Atalhos (F6) e preferencias de estudo — guardados na tabela `settings`
//! (key/value) do banco do usuario (ver [`crate::db`]) para nao precisar de
//! outro arquivo nem de outra migration.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::db;
use crate::error::Result;
use crate::hotkeys;

const CHAVE_LOOKUP: &str = "shortcut.lookup";
const CHAVE_CARD: &str = "shortcut.card";
const CHAVE_NOVOS_POR_DIA: &str = "review.newPerDay";
const CHAVE_ONBOARDING: &str = "app.onboardingDone";

/// Quantos cards novos a fila do dia introduz (F5). Quinze e o padrao do doc
/// 03; o teto existe para o usuario nao se afogar num deck grande de uma vez.
const NOVOS_POR_DIA_PADRAO: u32 = 15;
const NOVOS_POR_DIA_MAX: u32 = 200;

/// Os dois atalhos configuraveis hoje. `Esc` (fechar o card) fica de fora de
/// proposito: e o unico sem modificador, nunca colide com estes dois, e seu
/// registro ja e dinamico por conta propria (so existe com o card aberto —
/// ver `hotkeys::sync_escape`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcuts {
    pub lookup: String,
    pub card: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            lookup: hotkeys::DEFAULT_LOOKUP_SHORTCUT.to_string(),
            card: hotkeys::DEFAULT_CARD_SHORTCUT.to_string(),
        }
    }
}

/// Preferencias de estudo e de primeira execucao.
///
/// Separadas dos atalhos porque mudar uma delas nao mexe em nada registrado no
/// Windows: e so gravar e a proxima leitura ve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    /// Cards novos por dia na fila de revisao.
    pub new_per_day: u32,
    /// O wizard de primeira execucao (F8) ja foi concluido?
    pub onboarding_done: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            new_per_day: NOVOS_POR_DIA_PADRAO,
            onboarding_done: false,
        }
    }
}

fn ler_chave(conexao: &Connection, chave: &str) -> Result<Option<String>> {
    Ok(conexao
        .query_row("SELECT value FROM settings WHERE key = ?1", [chave], |l| {
            l.get(0)
        })
        .optional()?)
}

fn gravar_chave(conexao: &Connection, chave: &str, valor: &str) -> Result<()> {
    conexao.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![chave, valor],
    )?;
    Ok(())
}

/// Le os atalhos salvos, ou os padroes se o usuario nunca mexeu neles.
pub fn ler_atalhos(app: &AppHandle) -> Result<Shortcuts> {
    let guarda = db::conexao(app)?;
    let conexao = guarda.as_ref().expect("conexao() sempre deixa Some");
    let padrao = Shortcuts::default();
    Ok(Shortcuts {
        lookup: ler_chave(conexao, CHAVE_LOOKUP)?.unwrap_or(padrao.lookup),
        card: ler_chave(conexao, CHAVE_CARD)?.unwrap_or(padrao.card),
    })
}

/// Valida e persiste. Nao re-registra os atalhos — quem chama decide isso
/// (ver `hotkeys::reregister`), porque o comando precisa do resultado da
/// re-registracao antes de confirmar sucesso a UI.
fn salvar_atalhos(app: &AppHandle, shortcuts: &Shortcuts) -> Result<()> {
    let guarda = db::conexao(app)?;
    let conexao = guarda.as_ref().expect("conexao() sempre deixa Some");
    gravar_chave(conexao, CHAVE_LOOKUP, &shortcuts.lookup)?;
    gravar_chave(conexao, CHAVE_CARD, &shortcuts.card)?;
    Ok(())
}

/// Le as preferencias, caindo no padrao para cada chave que o usuario nunca
/// tocou. Valor corrompido no banco tambem cai no padrao: preferencia ilegivel
/// nao pode impedir o app de abrir.
pub fn ler_preferencias(app: &AppHandle) -> Result<Preferences> {
    let guarda = db::conexao(app)?;
    let conexao = guarda.as_ref().expect("conexao() sempre deixa Some");
    let padrao = Preferences::default();
    Ok(Preferences {
        new_per_day: ler_chave(conexao, CHAVE_NOVOS_POR_DIA)?
            .and_then(|bruto| bruto.parse().ok())
            .map(|valor: u32| valor.clamp(1, NOVOS_POR_DIA_MAX))
            .unwrap_or(padrao.new_per_day),
        onboarding_done: ler_chave(conexao, CHAVE_ONBOARDING)?
            .map(|bruto| bruto == "1")
            .unwrap_or(padrao.onboarding_done),
    })
}

// ---------------------------------------------------------------------------
// Comandos
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn settings_get_shortcuts(app: AppHandle) -> Result<Shortcuts> {
    ler_atalhos(&app)
}

/// Valida, persiste e re-registra os atalhos globais sem reiniciar o app.
///
/// A validacao de sintaxe (`hotkeys::validar`) pega combinacoes malformadas
/// cedo; uma combinacao sintaticamente valida mas ja tomada por outro app so
/// falha dentro de `reregister`, no `RegisterHotKey` do Windows — e o erro
/// disso tambem volta para a UI, em vez de falhar em silencio (doc 03, F6).
#[tauri::command]
pub async fn settings_set_shortcuts(app: AppHandle, shortcuts: Shortcuts) -> Result<Shortcuts> {
    // Compara os `Shortcut` ja resolvidos, nao as strings: "Alt+X" e "Alt+KeyX"
    // sao a mesma combinacao para o Windows, mas strings diferentes.
    if hotkeys::validar(&shortcuts.lookup)? == hotkeys::validar(&shortcuts.card)? {
        return Err(crate::error::Error::Platform(
            "espiar e abrir o card nao podem usar a mesma combinacao".into(),
        ));
    }
    hotkeys::reregister(&app, shortcuts.clone())?;
    salvar_atalhos(&app, &shortcuts)?;
    Ok(shortcuts)
}

#[tauri::command]
pub async fn settings_get_preferences(app: AppHandle) -> Result<Preferences> {
    ler_preferencias(&app)
}

/// Grava as preferencias e devolve o que ficou salvo — o `new_per_day` volta
/// ja limitado, entao a UI mostra o valor real em vez do que foi digitado.
#[tauri::command]
pub async fn settings_set_preferences(
    app: AppHandle,
    preferences: Preferences,
) -> Result<Preferences> {
    let ajustado = Preferences {
        new_per_day: preferences.new_per_day.clamp(1, NOVOS_POR_DIA_MAX),
        onboarding_done: preferences.onboarding_done,
    };
    let guarda = db::conexao(&app)?;
    let conexao = guarda.as_ref().expect("conexao() sempre deixa Some");
    gravar_chave(
        conexao,
        CHAVE_NOVOS_POR_DIA,
        &ajustado.new_per_day.to_string(),
    )?;
    gravar_chave(
        conexao,
        CHAVE_ONBOARDING,
        if ajustado.onboarding_done { "1" } else { "0" },
    )?;
    Ok(ajustado)
}

#[tauri::command]
pub async fn settings_reset_shortcuts(app: AppHandle) -> Result<Shortcuts> {
    let padrao = Shortcuts::default();
    hotkeys::reregister(&app, padrao.clone())?;
    salvar_atalhos(&app, &padrao)?;
    Ok(padrao)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memoria() -> Connection {
        let conexao = Connection::open_in_memory().expect("abriu em memoria");
        db::preparar(&conexao).expect("migrou");
        conexao
    }

    #[test]
    fn le_padroes_quando_nada_foi_salvo() {
        let conexao = memoria();
        let padrao = Shortcuts::default();
        assert_eq!(ler_chave(&conexao, CHAVE_LOOKUP).unwrap(), None);
        assert_eq!(padrao.lookup, hotkeys::DEFAULT_LOOKUP_SHORTCUT);
        assert_eq!(padrao.card, hotkeys::DEFAULT_CARD_SHORTCUT);
    }

    #[test]
    fn gravar_e_ler_de_volta_bate() {
        let conexao = memoria();
        gravar_chave(&conexao, CHAVE_LOOKUP, "Alt+KeyZ").unwrap();
        assert_eq!(
            ler_chave(&conexao, CHAVE_LOOKUP).unwrap(),
            Some("Alt+KeyZ".to_string())
        );
    }

    #[test]
    fn gravar_de_novo_sobrescreve_em_vez_de_duplicar() {
        let conexao = memoria();
        gravar_chave(&conexao, CHAVE_LOOKUP, "Alt+KeyZ").unwrap();
        gravar_chave(&conexao, CHAVE_LOOKUP, "Alt+KeyQ").unwrap();
        assert_eq!(
            ler_chave(&conexao, CHAVE_LOOKUP).unwrap(),
            Some("Alt+KeyQ".to_string())
        );
    }
}
