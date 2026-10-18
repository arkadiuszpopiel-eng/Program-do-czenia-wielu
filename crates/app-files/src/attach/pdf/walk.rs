//! Interpreter treści stron (tylko tekst): stan graficzny (`q`/`Q`/`cm`), stan tekstu (`Tf`, `Tc`,
//! `Tw`, `Tz`, `TL`, `Ts`), pozycjonowanie (`Td`, `TD`, `Tm`, `T*`), wypisywanie (`Tj`, `TJ`, `'`,
//! `"`) i formularze (`Do`) z limitem głębokości. Obrazy tylko odnotowujemy (rozpoznanie skanu).
//! Budżety: operacje, rozpakowane bajty, liczba formularzy i termin — sprawdzane w pętli.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use lopdf::{DecompressError, Dictionary, Document, Error, ObjectId};

use super::fonts::Font;
use super::lexer::{Lexer, Tok};
use super::load::{get, get_dict, get_name, num, page_resources, resource};
use super::sink::{Pen, Sink};
use super::state::{Arg, Gs, M, State, last_name, last_str, matrix, nums, read_array, skip_dict};
use super::{Limits, Stop};

/// Najwięcej operandów pamiętanych przed operatorem.
const MAX_ARGS: usize = 16;
/// Najgłębszy stos `q`.
const MAX_SAVES: usize = 64;
/// Najwięcej czcionek w pamięci podręcznej dokumentu.
const MAX_FONTS: usize = 512;
/// Największy strumień czcionki (`ToUnicode`, CMap kodowania).
const FONT_STREAM: usize = 1024 * 1024;

/// Przejście po stronach dokumentu.
pub(super) struct Walker<'d> {
    doc: &'d Document,
    limits: &'d Limits,
    deadline: Instant,
    fonts: HashMap<ObjectId, Rc<Font>>,
    ops: usize,
    budget: usize,
    forms: usize,
    page_image: bool,
    /// Strony pominięte, bo ich treść przekracza limit rozpakowanych danych.
    pub(super) skipped: usize,
    /// Powód przerwania (limit).
    pub(super) stop: Option<Stop>,
    /// Tekst.
    pub(super) sink: Sink,
}

impl<'d> Walker<'d> {
    /// Przejście z limitami; tekst najwyżej `chars` znaków.
    pub(super) fn new(
        doc: &'d Document,
        limits: &'d Limits,
        deadline: Instant,
        chars: usize,
    ) -> Self {
        Self {
            doc,
            limits,
            deadline,
            fonts: HashMap::new(),
            ops: 0,
            budget: limits.content_total,
            forms: 0,
            page_image: false,
            skipped: 0,
            stop: None,
            sink: Sink::new(chars),
        }
    }

    /// Czyta stronę `page` i zamyka ją w tekście jako stronę `number`; `true` — strona ma obrazy.
    pub(super) fn page(&mut self, number: u32, page: ObjectId) -> bool {
        self.page_image = false;
        if let Ok(dict) = self.doc.get_dictionary(page) {
            let resources = page_resources(self.doc, dict);
            let cap = self.budget.min(self.limits.page_content);
            match self.doc.get_page_content_with_limit(page, cap) {
                Ok(data) => {
                    self.budget = self.budget.saturating_sub(data.len());
                    self.run(&data, resources, Gs::new(M::ID), 0);
                }
                Err(_) if cap < self.limits.page_content => self.stop = Some(Stop::Content),
                Err(_) => {
                    self.skipped += 1;
                    self.sink
                        .note("[treść strony pominięta — przekracza limit rozpakowanych danych]");
                }
            }
        }
        self.sink.page_done(number, self.page_image);
        self.page_image
    }

    fn halted(&self) -> bool {
        self.stop.is_some() || self.sink.full
    }

    fn run(&mut self, data: &[u8], res: Option<&'d Dictionary>, gs: Gs, depth: usize) {
        let mut lex = Lexer::new(data);
        let mut st = State {
            gs,
            saved: Vec::new(),
            overflow: 0,
            tm: M::ID,
            tlm: M::ID,
        };
        let mut args: Vec<Arg<'_>> = Vec::new();
        let mut tokens = 0usize;
        while let Some(tok) = lex.next() {
            tokens += 1;
            if tokens.is_multiple_of(65_536) && Instant::now() >= self.deadline {
                self.stop = Some(Stop::Time);
            }
            if self.halted() {
                return;
            }
            let arg = match tok {
                Tok::Num(n) => Arg::Num(n),
                Tok::Name(n) => Arg::Name(n),
                Tok::Str(s) => Arg::Str(s),
                Tok::ArrOpen => Arg::Arr(read_array(&mut lex)),
                Tok::DictOpen => {
                    skip_dict(&mut lex);
                    Arg::Other
                }
                Tok::ArrClose | Tok::DictClose => continue,
                Tok::Op(op) => {
                    self.ops += 1;
                    if self.ops > self.limits.max_ops {
                        self.stop = Some(Stop::Ops);
                    } else if self.ops.is_multiple_of(1024) && Instant::now() >= self.deadline {
                        self.stop = Some(Stop::Time);
                    } else if op == b"BI" {
                        self.inline_image(&mut lex);
                    } else {
                        self.op(op, &args, &mut st, res, depth);
                    }
                    args.clear();
                    continue;
                }
            };
            if args.len() >= MAX_ARGS {
                args.drain(..MAX_ARGS / 2);
            }
            args.push(arg);
        }
    }

    fn inline_image(&mut self, lex: &mut Lexer<'_>) {
        self.page_image = true;
        while let Some(tok) = lex.next() {
            if matches!(tok, Tok::Op(b"ID")) {
                lex.skip_inline_image();
                return;
            }
        }
    }

    fn op(
        &mut self,
        op: &[u8],
        args: &[Arg<'_>],
        st: &mut State,
        res: Option<&'d Dictionary>,
        depth: usize,
    ) {
        match op {
            b"q" if st.saved.len() < MAX_SAVES => st.saved.push(st.gs.clone()),
            b"q" => st.overflow += 1,
            b"Q" if st.overflow > 0 => st.overflow -= 1,
            b"Q" => {
                if let Some(gs) = st.saved.pop() {
                    st.gs = gs;
                }
            }
            b"cm" => {
                if let Some(m) = matrix(args) {
                    st.gs.ctm = m.mul(st.gs.ctm);
                }
            }
            b"BT" => {
                st.tm = M::ID;
                st.tlm = M::ID;
            }
            b"Tf" => {
                if let (Some(name), Some([size])) = (last_name(args), nums::<1>(args)) {
                    st.gs.font = self.font(res, name);
                    st.gs.size = size;
                }
            }
            b"Tc" | b"Tw" | b"Tz" | b"TL" | b"Ts" => {
                if let Some([v]) = nums::<1>(args) {
                    match op {
                        b"Tc" => st.gs.tc = v,
                        b"Tw" => st.gs.tw = v,
                        b"Tz" => st.gs.th = v / 100.0,
                        b"TL" => st.gs.tl = v,
                        _ => st.gs.rise = v,
                    }
                }
            }
            b"Td" | b"TD" => {
                if let Some([x, y]) = nums::<2>(args) {
                    if op == b"TD" {
                        st.gs.tl = -y;
                    }
                    st.td(x, y);
                }
            }
            b"Tm" => {
                if let Some(m) = matrix(args) {
                    st.tm = m;
                    st.tlm = m;
                }
            }
            b"T*" => st.td(0.0, -st.gs.tl),
            b"Tj" => self.show(st, last_str(args)),
            b"'" => {
                st.td(0.0, -st.gs.tl);
                self.show(st, last_str(args));
            }
            b"\"" => {
                if let (Some([aw, ac]), Some(Arg::Str(s))) = (
                    args.len()
                        .checked_sub(1)
                        .and_then(|n| nums::<2>(args.get(..n)?)),
                    args.last(),
                ) {
                    st.gs.tw = aw;
                    st.gs.tc = ac;
                    st.td(0.0, -st.gs.tl);
                    self.show(st, Some(s));
                }
            }
            b"TJ" => {
                let Some(Arg::Arr(items)) = args.last() else {
                    return;
                };
                for item in items {
                    match item {
                        Arg::Str(s) => self.show(st, Some(s)),
                        Arg::Num(n) => {
                            let tx = -n / 1000.0 * st.gs.size * st.gs.th;
                            st.tm = M::translate(tx, 0.0).mul(st.tm);
                        }
                        _ => {}
                    }
                }
            }
            b"Do" => {
                if let Some(name) = last_name(args) {
                    self.xobject(res, name, st, depth);
                }
            }
            _ => {}
        }
    }

    fn font(&mut self, res: Option<&'d Dictionary>, name: &[u8]) -> Option<Rc<Font>> {
        let (id, obj) = resource(self.doc, res, b"Font", name)?;
        let dict = obj.as_dict().ok()?;
        if let Some(found) = id.and_then(|id| self.fonts.get(&id)) {
            return Some(found.clone());
        }
        let font = Rc::new(Font::build(self.doc, dict, &mut self.budget, FONT_STREAM));
        if let Some(id) = id
            && self.fonts.len() < MAX_FONTS
        {
            self.fonts.insert(id, font.clone());
        }
        Some(font)
    }

    fn show(&mut self, st: &mut State, bytes: Option<&[u8]>) {
        let (Some(font), Some(bytes)) = (st.gs.font.clone(), bytes) else {
            return;
        };
        let (x, y) = st.trm().origin();
        self.sink.start(x, y);
        let (size, th, tc, tw) = (st.gs.size, st.gs.th, st.gs.tc, st.gs.tw);
        let sink = &mut self.sink;
        font.glyphs(bytes, |g| {
            if let Some(text) = &g.text {
                sink.push(text);
            }
            let advance = (g.width * size + tc + if g.space { tw } else { 0.0 }) * th;
            st.tm = M::translate(advance, 0.0).mul(st.tm);
        });
        let trm = st.trm();
        let (ex, ey) = trm.origin();
        let [a, b, c, d, _, _] = trm.0;
        let len = a.hypot(b);
        let (ux, uy) = if len > 1e-9 {
            (a / len, b / len)
        } else {
            (1.0, 0.0)
        };
        self.sink.end(Pen {
            x: ex,
            y: ey,
            ux,
            uy,
            size: c.hypot(d),
        });
    }

    fn xobject(&mut self, res: Option<&'d Dictionary>, name: &[u8], st: &State, depth: usize) {
        let Some((_, obj)) = resource(self.doc, res, b"XObject", name) else {
            return;
        };
        let Ok(stream) = obj.as_stream() else { return };
        match get_name(self.doc, &stream.dict, b"Subtype") {
            Some(b"Image") => self.page_image = true,
            Some(b"Form")
                if depth < self.limits.form_depth && self.forms < self.limits.max_forms =>
            {
                self.forms += 1;
                let cap = self.budget.min(self.limits.page_content);
                let data = match stream.get_plain_content_with_limit(cap) {
                    Ok(data) => data,
                    Err(Error::Decompress(DecompressError::MemoryLimitExceeded { .. }))
                        if cap < self.limits.page_content =>
                    {
                        self.stop = Some(Stop::Content);
                        return;
                    }
                    Err(_) => return,
                };
                self.budget = self.budget.saturating_sub(data.len());
                let form = get(self.doc, &stream.dict, b"Matrix")
                    .and_then(|o| o.as_array().ok())
                    .and_then(|a| {
                        let v: Vec<f64> = a.iter().filter_map(|o| num(self.doc, o)).collect();
                        <[f64; 6]>::try_from(v).ok().map(M)
                    })
                    .unwrap_or(M::ID);
                let form_res = get_dict(self.doc, &stream.dict, b"Resources").or(res);
                let mut gs = st.gs.clone();
                gs.ctm = form.mul(st.gs.ctm);
                self.run(&data, form_res, gs, depth + 1);
            }
            _ => {}
        }
    }
}
