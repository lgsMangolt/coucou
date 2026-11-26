// Coucou's memory of the print shop's customers, kept as an Obsidian vault (Documents\Coucou\Memória):
// one note per customer, with what they ordered, written by Coucou, and a "Notas" part that is the
// shop's own: Coucou reads it and never touches it. `.coucou/clientes.json` in the vault holds the
// same history for the code (Obsidian does not show dot folders).
//
// Only what Coucou saw while running is kept: the orders, never the conversations. Everything stays
// on this computer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

/// Orders kept per customer: the oldest go.
const MAX_ENTRIES: usize = 60;
/// The orders shown to the model.
const CONTEXT_ENTRIES: usize = 5;
/// The shop's notes shown to the model.
const MAX_NOTES_CHARS: usize = 600;
const START: &str = "<!-- coucou:inicio · daqui para baixo o Coucou reescreve sozinho -->";
const END: &str = "<!-- coucou:fim -->";
const NOTES_HINT: &str = "_Escreva aqui o que o Coucou deve saber deste cliente (ex.: \"sempre frente e verso\", \"paga na retirada\"). Ele lê esta parte e nunca apaga._";

/// The vault.
pub fn vault() -> PathBuf {
    crate::platform::home_dir().join("Documents").join("Coucou").join("Memória")
}

/// One order, as the customer's note shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The message that carried the customer's file: the same order read again keeps its line.
    pub key: String,
    /// "2026-11-26"
    pub data: String,
    /// The table's line ("impressão P&B, sulfite 75g"), or "".
    pub item: String,
    /// "6 × impressão colorida, sulfite 75g + corte", or the question asked.
    pub pedido: String,
    pub total: Option<i64>,
    /// "Orçamento enviado", "Pago", "Impresso"…
    pub situacao: String,
    pub arquivo: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Client {
    pub chat: String,
    pub nome: String,
    pub telefone: String,
    /// The note's file name in the vault, chosen once.
    pub nota: String,
    pub primeiro: String,
    pub ultimo: String,
    pub pedidos: Vec<Entry>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    clientes: Vec<Client>,
}

/// One writer at a time (both WhatsApp accounts write here).
static WRITING: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
/// What was last written for each order, so an unchanged order is not written again.
static SEEN: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Notes an order of a customer, in the background, when it changed since it was last noted.
pub fn remember(chat: String, name: String, phone: String, entry: Entry) {
    {
        let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
        let key = format!("{chat}~{}", entry.key);
        if seen.get(&key) == Some(&entry) {
            return;
        }
        if seen.len() > 5000 {
            seen.clear();
        }
        seen.insert(key, entry.clone());
    }
    std::thread::spawn(move || {
        let _one = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
        if let Err(err) = write_in(&vault(), &chat, &name, &phone, entry) {
            crate::log::line(format!("memoria: could not write a customer's note: {err}"));
        }
    });
}

/// Writes a note of Coucou's own (a print shop job's sheet) in the background, at `relative` in the
/// vault. Only inside the vault.
pub fn write_note(relative: String, content: String) {
    let path = vault().join(&relative);
    if !path.starts_with(vault()) || relative.contains("..") {
        return;
    }
    std::thread::spawn(move || {
        let _one = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
        let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| write_atomic(&path, content.as_bytes()));
        if let Err(err) = written {
            crate::log::line(format!("memoria: could not write a job's note: {err}"));
        }
    });
}

/// The shop's approved answers (the vault note "Loja": address, hours, Pix…), without the lines the
/// owner left blank; None when there are none. Read as data by the model.
pub fn shop_facts() -> Option<String> {
    std::fs::read_to_string(vault().join("Loja.md")).ok().and_then(|note| facts_of(&note))
}

fn facts_of(note: &str) -> Option<String> {
    let body = match note.strip_prefix("---") {
        Some(rest) => rest.split_once("\n---").map_or(note, |(_, after)| after),
        None => note,
    };
    let facts: Vec<String> = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("- ") || l.starts_with("* "))
        .map(|l| l[2..].replace("**", ""))
        .filter(|l| l.split_once(':').is_some_and(|(_, v)| !v.trim().is_empty()))
        .collect();
    let text: String = facts.join("\n").chars().take(1500).collect();
    (!text.is_empty()).then_some(text)
}

/// The log is set aside past this size (registro.jsonl → registro.antigo.jsonl).
const MAX_LOG_BYTES: u64 = 5_000_000;

/// One line of `.coucou/registro.jsonl`, for the improvement cycle (evals/colher.py): what Coucou
/// said or suggested in a conversation, and what you sent there. Stays on this computer.
pub fn log_event(account: u8, chat: String, kind: &'static str, text: String, conversation: Vec<String>) {
    std::thread::spawn(move || {
        let _one = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
        let dir = vault().join(".coucou");
        let path = dir.join("registro.jsonl");
        let line = serde_json::json!({
            "quando": crate::whatsapp::now_ms(), "conta": account, "chat": chat, "tipo": kind, "texto": text, "conversa": conversation,
        });
        let written = std::fs::create_dir_all(&dir).and_then(|()| {
            if std::fs::metadata(&path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
                std::fs::rename(&path, dir.join("registro.antigo.jsonl"))?;
            }
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
            writeln!(file, "{line}")
        });
        if let Err(err) = written {
            crate::log::line(format!("memoria: could not write the log: {err}"));
        }
    });
}

/// What the model is told about this customer, or None for someone new.
pub fn context(chat: &str) -> Option<String> {
    let _one = WRITING.lock().unwrap_or_else(PoisonError::into_inner);
    context_in(&vault(), chat)
}

fn store_path(base: &Path) -> PathBuf {
    base.join(".coucou").join("clientes.json")
}

fn load_store(base: &Path) -> Store {
    std::fs::read(store_path(base)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn safe(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '#' | '^' | '[' | ']') { ' ' } else { c })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(60).collect()
}

fn digits_end(phone: &str) -> String {
    let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
    digits[digits.len().saturating_sub(4)..].to_string()
}

/// "Ana (final 1234).md", never one another customer already has.
fn note_name(store: &Store, name: &str, phone: &str) -> String {
    let base = match (safe(name), digits_end(phone)) {
        (n, d) if !n.is_empty() && !d.is_empty() => format!("{n} (final {d})"),
        (n, _) if !n.is_empty() => n,
        (_, d) if !d.is_empty() => format!("Cliente (final {d})"),
        _ => "Cliente".to_string(),
    };
    let taken = |candidate: &str| store.clientes.iter().any(|c| c.nota.eq_ignore_ascii_case(candidate));
    let mut candidate = format!("{base}.md");
    let mut n = 2;
    while taken(&candidate) {
        candidate = format!("{base} {n}.md");
        n += 1;
    }
    candidate
}

fn write_in(base: &Path, chat: &str, name: &str, phone: &str, entry: Entry) -> std::io::Result<()> {
    let mut store = load_store(base);
    let index = match store.clientes.iter().position(|c| c.chat == chat) {
        Some(i) => i,
        None => {
            let nota = note_name(&store, name, phone);
            store.clientes.push(Client { chat: chat.into(), nota, ..Client::default() });
            store.clientes.len() - 1
        }
    };
    let client = &mut store.clientes[index];
    if !name.trim().is_empty() {
        client.nome = name.trim().to_string();
    }
    client.telefone = phone.to_string();
    if client.primeiro.is_empty() {
        client.primeiro = entry.data.clone();
    }
    client.ultimo = entry.data.clone();
    match client.pedidos.iter_mut().find(|e| e.key == entry.key) {
        Some(old) => *old = entry,
        None => client.pedidos.push(entry),
    }
    if client.pedidos.len() > MAX_ENTRIES {
        let extra = client.pedidos.len() - MAX_ENTRIES;
        client.pedidos.drain(0..extra);
    }
    let client = client.clone();
    std::fs::create_dir_all(base.join(".coucou"))?;
    std::fs::create_dir_all(base.join("Clientes"))?;
    let json = serde_json::to_vec_pretty(&store).map_err(std::io::Error::other)?;
    write_atomic(&store_path(base), &json)?;
    let path = base.join("Clientes").join(&client.nota);
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    write_atomic(&path, render(&client, &old).as_bytes())?;
    ensure_home(base)
}

/// Written next to the file, then put in its place: Obsidian never reads half a note.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp~");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn yaml(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn cell(text: &str) -> String {
    text.replace('|', "/").replace(['\n', '\r'], " ")
}

/// 123456 → "R$ 1.234,56".
fn reais(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.unsigned_abs();
    let whole = (cents / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    format!("{sign}R$ {grouped},{:02}", cents % 100)
}

fn date_br(iso: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    if let [y, m, d] = parts[..] { format!("{d}/{m}/{y}") } else { iso.to_string() }
}

/// What the customer orders most, when it is at least twice: ("impressão P&B, sulfite 75g", 3).
fn usual(client: &Client) -> Option<(String, usize)> {
    let mut count: Vec<(String, usize)> = Vec::new();
    for e in client.pedidos.iter().filter(|e| !e.item.is_empty()) {
        match count.iter_mut().find(|(i, _)| *i == e.item) {
            Some((_, n)) => *n += 1,
            None => count.push((e.item.clone(), 1)),
        }
    }
    count.into_iter().filter(|(_, n)| *n >= 2).max_by_key(|(_, n)| *n)
}

/// The shop's own part of a note: everything between the front matter and Coucou's block.
fn own_part(old: &str) -> Option<String> {
    let body = match old.strip_prefix("---") {
        Some(rest) => rest.split_once("\n---").map(|(_, after)| after.trim_start_matches(['\r', '\n'])).unwrap_or(old),
        None => old,
    };
    let own = body.split(START).next().unwrap_or(body).trim_end();
    (!own.trim().is_empty()).then(|| own.to_string())
}

/// The note of one customer: the front matter and Coucou's block are rewritten, the rest is kept.
pub fn render(client: &Client, old: &str) -> String {
    let title = if client.nome.is_empty() { client.telefone.clone() } else { client.nome.clone() };
    let own = own_part(old).unwrap_or_else(|| format!("# {title}\n\n## Notas\n{NOTES_HINT}"));
    let mut out = format!(
        "---\ncliente: {}\ntelefone: {}\nprimeiro_contato: {}\nultimo_contato: {}\npedidos: {}\ntags: [cliente]\n---\n{own}\n\n{START}\n",
        yaml(&client.nome),
        yaml(&client.telefone),
        client.primeiro,
        client.ultimo,
        client.pedidos.len()
    );
    if let Some((item, n)) = usual(client) {
        out.push_str(&format!("## Costuma pedir\n- {item} ({n} pedidos)\n\n"));
    }
    out.push_str("## Pedidos\n| Data | Pedido | Total | Situação | Arquivo |\n|---|---|---|---|---|\n");
    for e in client.pedidos.iter().rev() {
        let total = e.total.map(reais).unwrap_or_else(|| "—".into());
        out.push_str(&format!("| {} | {} | {} | {} | {} |\n", date_br(&e.data), cell(&e.pedido), total, cell(&e.situacao), cell(&e.arquivo)));
    }
    out.push_str(END);
    out.push('\n');
    out
}

/// The shop's notes on a customer, without the hint.
fn notes_of(note: &str) -> String {
    let own = own_part(note).unwrap_or_default();
    let Some((_, after)) = own.split_once("## Notas") else { return String::new() };
    let text: String = after
        .lines()
        .take_while(|l| !l.starts_with("## "))
        .filter(|l| l.trim() != NOTES_HINT)
        .collect::<Vec<_>>()
        .join(" ");
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NOTES_CHARS).collect()
}

fn context_in(base: &Path, chat: &str) -> Option<String> {
    let store = load_store(base);
    let client = store.clientes.iter().find(|c| c.chat == chat)?;
    let notes = std::fs::read_to_string(base.join("Clientes").join(&client.nota)).map(|n| notes_of(&n)).unwrap_or_default();
    if client.pedidos.is_empty() && notes.is_empty() {
        return None;
    }
    let mut out = vec![format!("Customer since {} ({} orders).", date_br(&client.primeiro), client.pedidos.len())];
    if let Some((item, n)) = usual(client) {
        out.push(format!("Usually orders: {item} ({n} times)."));
    }
    let recent: Vec<String> = client
        .pedidos
        .iter()
        .rev()
        .take(CONTEXT_ENTRIES)
        .map(|e| format!("- {}: {} ({})", date_br(&e.data), e.pedido, e.situacao))
        .collect();
    if !recent.is_empty() {
        out.push(format!("Last orders:\n{}", recent.join("\n")));
    }
    if !notes.is_empty() {
        out.push(format!("The shop's notes on this customer: {notes}"));
    }
    Some(out.join("\n"))
}

/// Makes the vault and its first page, once.
pub fn ensure_vault() {
    let base = vault();
    if let Err(err) = std::fs::create_dir_all(base.join("Clientes")).and_then(|()| ensure_home(&base)) {
        crate::log::line(format!("memoria: could not make the vault: {err}"));
    }
}

/// The vault's first page, written once.
fn ensure_home(base: &Path) -> std::io::Result<()> {
    let path = base.join("Início.md");
    if path.exists() {
        return Ok(());
    }
    write_atomic(
        &path,
        "# Memória do Coucou\n\n\
O Coucou anota aqui cada cliente que faz pedido de impressão pelo WhatsApp.\n\n\
- [[Tabela de preços]]: os preços que o Coucou usa. Mude os valores ali; vale no próximo pedido.\n\
- **Clientes/**: uma nota por cliente. A parte **Notas** é sua: o Coucou lê antes de responder e nunca apaga. \
Da linha `coucou:inicio` para baixo, ele reescreve sozinho.\n\n\
Nada daqui sai deste computador.\n"
            .as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("coucou-memoria-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    fn entry(key: &str, data: &str, item: &str, situacao: &str) -> Entry {
        Entry {
            key: key.into(),
            data: data.into(),
            item: item.into(),
            pedido: format!("6 × {item}"),
            total: Some(1200),
            situacao: situacao.into(),
            arquivo: "a.pdf".into(),
        }
    }

    #[test]
    fn a_customer_gets_one_note_and_the_shops_notes_are_kept() {
        let base = temp();
        let chat = "5554999990000@s.whatsapp.net";
        write_in(&base, chat, "Ana", "+5554999990000", entry("m1", "2026-11-25", "impressão P&B, sulfite 75g", "Orçamento enviado")).unwrap();
        let path = base.join("Clientes").join("Ana (final 0000).md");
        let note = std::fs::read_to_string(&path).unwrap();
        assert!(note.contains("cliente: \"Ana\"") && note.contains("## Notas") && note.contains("| 25/11/2026 | 6 × impressão P&B, sulfite 75g | R$ 12,00 | Orçamento enviado | a.pdf |"), "{note}");
        assert!(base.join("Início.md").exists());

        // The shop writes its notes; Coucou rewrites only its block.
        let edited = note.replace(NOTES_HINT, "Sempre frente e verso. Paga na retirada.");
        std::fs::write(&path, edited).unwrap();
        write_in(&base, chat, "Ana", "+5554999990000", entry("m1", "2026-11-25", "impressão P&B, sulfite 75g", "Pago")).unwrap();
        write_in(&base, chat, "Ana", "+5554999990000", entry("m2", "2026-11-26", "impressão P&B, sulfite 75g", "Impresso")).unwrap();
        let note = std::fs::read_to_string(&path).unwrap();
        assert!(note.contains("Sempre frente e verso. Paga na retirada."), "{note}");
        assert_eq!(note.matches("| 25/11/2026 |").count(), 1, "the same order is one line: {note}");
        assert!(note.contains("| Pago |") && note.contains("| Impresso |") && note.contains("## Costuma pedir\n- impressão P&B, sulfite 75g (2 pedidos)"));
        assert_eq!(note.matches(START).count(), 1);

        let context = context_in(&base, chat).unwrap();
        assert!(context.contains("Usually orders: impressão P&B, sulfite 75g (2 times)."), "{context}");
        assert!(context.contains("The shop's notes on this customer: Sempre frente e verso. Paga na retirada."), "{context}");
        assert!(context_in(&base, "outro@s.whatsapp.net").is_none(), "another customer's memory is never shown");

        // Another Ana gets her own note.
        write_in(&base, "5554888880000@s.whatsapp.net", "Ana", "+5554888880000", entry("x", "2026-11-26", "", "Com o gerente")).unwrap();
        assert!(base.join("Clientes").join("Ana (final 0000) 2.md").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_shops_blank_answers_are_left_out() {
        let note = "---\ntags: [loja]\n---\n# Loja\n\nTexto.\n\n- **Endereço:** Rua Pio XII, 285\n- **Horário:** \n- **Pix:** CNPJ 1\n";
        assert_eq!(facts_of(note).as_deref(), Some("Endereço: Rua Pio XII, 285\nPix: CNPJ 1"));
        assert_eq!(facts_of("# Loja\n- **Horário:**  \n"), None);
    }

    #[test]
    fn names_are_safe_file_names() {
        let store = Store::default();
        assert_eq!(note_name(&store, "Zé: a/b|c", "+55 54 9999-1234"), "Zé a b c (final 1234).md");
        assert_eq!(note_name(&store, "", "+5554999991234"), "Cliente (final 1234).md");
        assert_eq!(notes_of(&format!("# Ana\n\n## Notas\n{NOTES_HINT}\n\n{START}\n## Pedidos")), "");
    }
}
