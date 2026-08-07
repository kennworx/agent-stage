//! The served review page: the document, plus everything a reviewer acts with.
//!
//! The difference from a baked page is the return leg. A baked file is read-only
//! because it has no server; a served one has `/feedback` and `/finish` behind it,
//! so every block that takes feedback carries a form, and what has already been
//! recorded is drawn back onto the page.
//!
//! **Recorded feedback is rendered here, not fetched.** The alternative — ship the
//! bare document and let a script pull `/state` and build the cards — would mean
//! two implementations of one card, in two languages, drifting. The server knows
//! what was said; it can say it.
//!
//! **Almost nothing needs a script.** A comment and an answer are a form post, and
//! the server answers with a redirect to the block, so the page works with
//! scripting off. Two things genuinely cannot be declared: aiming a comment at a
//! diagram node the reader clicked, and telling the host the tab closed. Those are
//! [`SCRIPT`], and they are the whole of it.

use crate::block::Block;
use crate::parse::parse_artifact;
use ags_feedback::{FeedbackItem, FeedbackKind, SubTarget};

use super::blocks::{block_anchor_id, escape};
use super::document::{chrome, chrome_css, rail_of, shell, Rendered};

/// What the reviewer can do that the page alone cannot express.
///
/// Node targeting: a click inside an inline SVG has to find the `[data-id]` it
/// landed on, open that block's composer, put the id into its form, and say so on
/// screen. Nothing declarative reaches inside a drawing.
///
/// Closing clears it. The target used to survive the composer being shut, so
/// pressing the block's own Comment button afterwards reopened a composer still
/// aimed at whichever node had last been clicked — the control said "comment on
/// this block" and would have filed the note against a node instead.
///
/// The heartbeat: while the page is open it pings `/state`, which is how the host
/// knows it is still there. This is the signal that actually decides the server's
/// lifetime — positive evidence of presence, on a schedule, rather than a
/// best-effort notice of absence that may never arrive or may arrive when nothing
/// is wrong.
///
/// The close beacon: when the tab goes, the host hears about it at once rather
/// than waiting out the idle window. `sendBeacon` is the only call that survives teardown —
/// a `fetch` is cancelled. `persisted` is skipped because a page going into the
/// back-forward cache has not closed.
///
/// Posting without leaving the page: a comment goes by `fetch` and the card that
/// comes back is inserted where it belongs, so the reviewer keeps their scroll
/// position and the document is not re-parsed. The form underneath is untouched —
/// with no script it posts normally and the host redirects — so this is an
/// enhancement of a working control rather than a replacement for one.
///
/// An answer is the same trade, one step further. Picking an option *is* the
/// answer, so there is nothing for a button to add: the script hides it and posts
/// on change instead. The button is still rendered, because without a script it is
/// the only way to submit one — hiding it is the enhancement, and a page that
/// never runs this keeps a control that works.
///
/// Nothing confirms it. The chosen option stays chosen, which is the state and
/// the receipt at once; a "saved" beside it says only what the control already
/// shows.
///
/// Finishing does not navigate either. It used to, and a reload threw away where
/// the reviewer was standing — the same fault a comment had, one control later.
/// The host answers with the finished notice; the page swaps it in and strips the
/// controls that no longer apply.
///
/// **A submit that does navigate is not the reviewer leaving**, and `going` is
/// what tells them apart. Nothing on this page navigates now, but the flag stays:
/// it is what makes the beacon mean "the tab is gone" rather than "something
/// submitted", and the next control added here should not have to rediscover
/// that.
const SCRIPT: &str = "\
function shut(d){\
var c=d.querySelector('.toggle');if(!c)return;\
c.checked=false;\
c.dispatchEvent(new Event('change',{bubbles:true}));\
}\
function post(to,body,after){\
fetch(to,{method:'POST',headers:{\
'Content-Type':'application/x-www-form-urlencoded','X-Ags-Fragment':'1'},\
body:body})\
.then(function(r){return r.ok?r.text():null;}).then(after);}\
function send(f,after){\
post('/feedback',new URLSearchParams(new FormData(f)).toString(),after);}\
document.addEventListener('click',function(e){\
var n=e.target.closest('[data-id]');if(!n)return;\
var s=n.closest('.block');if(!s)return;\
var d=s.querySelector('.annotate');if(!d)return;\
var f=d.querySelector('form');if(!f)return;\
var c=d.querySelector('.toggle');if(c)c.checked=true;\
f.sub.value=n.getAttribute('data-id');\
var t=d.querySelector('.aim');if(t){t.textContent=n.getAttribute('data-id');t.hidden=false;}\
f.body.focus();\
});\
document.addEventListener('change',function(e){\
var c=e.target;\
if(!c.matches||!c.matches('.annotate>.toggle'))return;\
var d=c.parentNode;\
if(c.checked){var t=d.querySelector('textarea');if(t)t.focus();return;}\
var f=d.querySelector('form');if(f&&f.sub)f.sub.value='';\
var a=d.querySelector('.aim');if(a)a.hidden=true;\
});\
document.addEventListener('submit',function(e){\
var f=e.target;\
var to=f.getAttribute('action');\
if(to==='/finish'){\
e.preventDefault();\
post('/finish','',function(html){\
if(html===null)return;\
var bar=document.querySelector('.done');\
if(bar)bar.outerHTML=html;\
document.querySelectorAll('.annotate').forEach(function(a){a.remove();});\
document.querySelectorAll('form.answer').forEach(function(g){\
g.querySelectorAll('input,button').forEach(function(i){i.disabled=true;});});\
});\
return;}\
if(to!=='/feedback'){going=false;return;}\
e.preventDefault();\
send(f,function(html){\
var s=f.closest('.block');if(!s)return;\
if(html){\
var n=s.querySelector('.notes');\
if(!n){n=document.createElement('div');n.className='notes';\
s.querySelector('.annotate').insertAdjacentElement('beforebegin',n);}\
n.insertAdjacentHTML('beforeend',html);}\
var t=f.querySelector('textarea');if(t)t.value='';\
if(f.sub)f.sub.value='';\
var a=f.querySelector('.aim');if(a)a.hidden=true;\
var w=s.querySelector('.annotate');if(w)shut(w);\
});\
});\
document.addEventListener('keydown',function(e){\
var t=e.target;\
if(!t||!t.closest)return;\
var d=t.closest('.annotate');if(!d)return;\
var c=d.querySelector('.toggle');if(!c||!c.checked)return;\
if(e.key==='Escape'){e.preventDefault();shut(d);if(c.focus)c.focus();}\
else if(e.key==='Enter'&&(e.ctrlKey||e.metaKey)){\
e.preventDefault();\
var f=d.querySelector('form');\
if(f){if(f.requestSubmit)f.requestSubmit();\
else f.dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));}}\
});\
document.querySelectorAll('form.answer').forEach(function(f){\
var b=f.querySelector('button');if(b)b.hidden=true;\
f.addEventListener('change',function(){send(f,function(){});});\
});\
setInterval(function(){fetch('/state').catch(function(){});},7000);\
var going=true;\
addEventListener('pagehide',function(e){\
if(going&&!e.persisted)navigator.sendBeacon&&navigator.sendBeacon('/shutdown');\
});";

/// The review-only rules: annotation cards, the composer, the finish bar.
const REVIEW_CSS: &str = "\
.notes{margin:.6rem 0 0;display:flex;flex-direction:column;gap:.4rem}\
.note-card{border:1px solid var(--border);border-left:3px solid var(--primary);\
border-radius:6px;padding:.5rem .7rem;background:var(--card);font-size:.9rem}\
.note-card .who{color:var(--muted-foreground);font-size:.78rem;display:block;\
margin-bottom:.15rem}\
/* A checkbox rather than `<details>`. The disclosure had the toggle pinned to the \
   front of the markup by the spec, so putting Cancel next to the button it \
   cancels meant reordering with flex — which did not survive contact with a real \
   page. A checkbox puts the control wherever the layout wants it, and is no less \
   declarative: this still opens, closes and posts with no script. */\
.annotate{margin:.5rem 0 0}\
.annotate>.toggle{position:absolute;opacity:0;width:0;height:0}\
.annotate .ask,.annotate .cancel{display:inline-block;cursor:pointer;\
font-size:.78rem;color:var(--muted-foreground);border:1px solid var(--border);\
border-radius:999px;padding:.3rem .9rem;user-select:none}\
.annotate .ask:hover,.annotate .cancel:hover{color:var(--foreground)}\
.annotate>.toggle:focus-visible~.ask{outline:2px solid var(--primary);\
outline-offset:2px}\
.annotate>.pane{display:none}\
.annotate>.toggle:checked~.ask{display:none}\
.annotate>.toggle:checked~.pane{display:block}\
/* Both controls on the left, under the corner of the field they act on, with the \
   one that posts first — it is what the reviewer came here to do. */\
.annotate .row{display:flex;justify-content:flex-start;align-items:center;\
gap:.5rem;margin-top:.5rem}\
.annotate .box{position:relative}\
.annotate textarea{display:block;width:100%;box-sizing:border-box;font:inherit;\
font-size:.9rem;color:var(--foreground);background:var(--background);\
border:1px solid var(--border);border-radius:6px;padding:.5rem .6rem;\
resize:vertical;min-height:3.4rem}\
/* The target rides in the field's own corner: it says what this comment is \
   about, so it belongs on the thing being written in, and it costs no space. */\
.aim{position:absolute;top:.4rem;right:.5rem;color:var(--primary);\
font-size:.72rem;background:var(--card);border:1px solid var(--primary);\
border-radius:999px;padding:.05rem .5rem;pointer-events:none}\
.question form.answer{display:flex;align-items:center;flex-wrap:wrap}\
.question form.answer ul{flex:0 0 100%}\
.aim{position:absolute;top:.4rem;right:.5rem;color:var(--primary);\
font-size:.72rem;background:var(--card);border:1px solid var(--primary);\
border-radius:999px;padding:.05rem .5rem;pointer-events:none}\
button{font:inherit;font-size:.82rem;color:var(--primary-foreground);\
background:var(--primary);border:0;border-radius:999px;padding:.35rem .9rem;\
cursor:pointer}\
button.quiet{background:var(--card);color:var(--muted-foreground);\
border:1px solid var(--border)}\
.done{margin:2.5rem 0 4rem;display:flex;align-items:center;gap:.7rem;\
color:var(--muted-foreground);font-size:.85rem}\
.ended{border:1px solid var(--primary);border-radius:8px;padding:.7rem 1rem;\
margin:2.5rem 0 4rem;color:var(--foreground);background:var(--card)}\
.node-target{cursor:pointer}\
[data-id]:hover>rect,[data-id]:hover>polygon,[data-id]:hover>circle,\
[data-id]:hover>ellipse{stroke:var(--primary);stroke-width:2}";

/// A block a reviewer may leave prose on.
///
/// Either verb counts. `annotate` points at something inside the block and
/// `comment` at the whole of it, but both end as a note filed against it, and the
/// composer is the same either way — a question that takes only `comment` needs
/// one exactly as a diagram that takes `annotate` does.
///
/// Read from the affordance table rather than restated, so the page cannot offer
/// a verb the catalog does not advertise to the agent.
fn takes_comment(block: &Block) -> bool {
    crate::affordances::affordances(&block.type_token)
        .iter()
        .any(|a| {
            matches!(
                a.verb,
                crate::affordances::Affordance::Annotate | crate::affordances::Affordance::Comment
            )
        })
}

/// One recorded note, drawn back onto the block it was left on.
///
/// Public because a scripted post is answered with exactly this: the host renders
/// the card and the page inserts it. One card, one implementation — the client
/// never learns how to draw one.
#[must_use]
pub fn note_card(item: &FeedbackItem) -> String {
    let who = match &item.sub_target {
        Some(SubTarget::Node(id)) => format!("on {}", escape(id)),
        Some(other) => escape(&other.describe()),
        None => "on this block".to_string(),
    };
    format!(
        "<div class=\"note-card\"><span class=\"who\">{who}</span>{}</div>",
        escape(&item.body)
    )
}

/// The notes already recorded against `block_id`, in the order they were left.
fn notes_for<'a>(feedback: &'a [FeedbackItem], block_id: &str) -> Vec<&'a FeedbackItem> {
    feedback
        .iter()
        .filter(|i| i.block_id == block_id && i.kind == FeedbackKind::Annotation)
        .collect()
}

/// The composer for one block: a note, optionally aimed at a node.
///
/// Folded away until asked for. A textarea under every block is nine textareas on
/// a page with nine blocks, which reads as a form to fill in rather than a
/// document to read — and most blocks will never be commented on.
/// A checkbox and two labels, not a script — the composer opens, closes and posts
/// on a page where the script never ran. `<details>` was the obvious choice and
/// the wrong one: it pins its summary to the front of the markup, so putting
/// *Cancel* beside the button it cancels meant reordering with flex, and that came
/// out as a narrow field wedged against one edge. A checkbox has no such rule, so
/// the controls are simply written where they belong.
///
/// Open, the field takes the full width and both controls sit under its left
/// edge, with the one that posts first. Clicking a diagram node opens it too, and
/// the keyboard reaches it — Escape closes, Control-Enter (or Command-Enter)
/// posts. Those are what the script adds to a control that already worked without
/// it.
///
/// Opening it puts the cursor in the field. Without that a reviewer has to click
/// twice to start typing, and Escape does nothing, because the key handler only
/// hears about a composer that holds the focus.
fn composer(block_id: &str) -> String {
    let id = escape(block_id);
    format!(
        "<div class=\"annotate\">\
         <input type=\"checkbox\" class=\"toggle\" id=\"c-{id}\">\
         <label class=\"ask\" for=\"c-{id}\">Comment</label>\
         <div class=\"pane\">\
         <form id=\"f-{id}\" method=\"post\" action=\"/feedback\">\
         <input type=\"hidden\" name=\"block_id\" value=\"{id}\">\
         <input type=\"hidden\" name=\"sub\" value=\"\">\
         <input type=\"hidden\" name=\"kind\" value=\"annotation\">\
         <div class=\"box\">\
         <textarea name=\"body\" rows=\"3\" placeholder=\"Comment on this block…\" \
         aria-label=\"Comment\"></textarea>\
         <span class=\"aim\" hidden></span></div></form>\
         <div class=\"row\">\
         <button type=\"submit\" form=\"f-{id}\" title=\"Ctrl+Enter\">Comment</button>\
         <label class=\"cancel\" for=\"c-{id}\">Cancel</label></div></div></div>"
    )
}

/// Everything a reviewer sees under a block: what was said, and the way to add.
fn under(block: &Block, feedback: &[FeedbackItem], locked: bool) -> String {
    let Some(id) = block.id.as_deref() else {
        return String::new();
    };
    let notes = notes_for(feedback, id);
    let cards = if notes.is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"notes\">{}</div>",
            notes.iter().map(|i| note_card(i)).collect::<String>()
        )
    };
    // A finished review keeps what was said and drops every way to say more.
    if locked || !takes_comment(block) {
        return cards;
    }
    format!("{cards}{}", composer(id))
}

/// The notice a finished review carries in place of its controls.
///
/// Public because a scripted finish is answered with exactly this, the same way a
/// scripted comment is answered with its card: the host renders, the page inserts,
/// and neither has to know how the other spells it.
#[must_use]
pub fn ended_notice() -> String {
    "<div class=\"ended\">This review is finished. \
     Everything recorded has gone back to the agent.</div>"
        .to_string()
}

/// The bar that ends the review, or the notice saying it already ended.
fn finish_bar(ended: bool) -> String {
    if ended {
        return ended_notice();
    }
    "<div class=\"done\"><form method=\"post\" action=\"/finish\">\
     <button type=\"submit\">Finish review</button></form>\
     <span>Sends everything recorded back to the agent and closes the review.</span>\
     </div>"
        .to_string()
}

/// Render the served review page.
///
/// `feedback` is what the session has settled so far and `ended` whether the
/// review is closed — both read fresh on each request, so a reload shows what was
/// just posted without the page having to remember anything.
#[must_use]
pub fn review(source: &str, name: &str, feedback: &[FeedbackItem], ended: bool) -> String {
    let artifact = parse_artifact(source);
    let Rendered {
        content,
        entries,
        themes,
    } = super::document::render_body(source, &artifact, feedback, !ended, |block| {
        under(block, feedback, ended)
    });
    let title = if name.is_empty() {
        "ags".to_string()
    } else {
        format!("ags · {name}")
    };
    shell(
        &title,
        &format!("{}{REVIEW_CSS}", chrome_css(&themes)),
        &format!("{}{}", chrome(&themes), rail_of(&entries)),
        &format!("{content}{}", finish_bar(ended)),
        Some(SCRIPT),
    )
}

/// The anchor a form post redirects back to, so the reviewer lands where they
/// were rather than at the top of the document.
#[must_use]
pub fn anchor_for(block_id: &str) -> String {
    format!("/#{}", block_anchor_id(block_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Review\n\n```mermaid #flow\ngraph TD\n  A --> B\n```\n\n\
```question #q1 type=radio\nWhich store?\n- Redis\n- SQLite\n```\n";

    fn note(block: &str, sub: Option<&str>, body: &str) -> FeedbackItem {
        FeedbackItem::new(
            block,
            sub.map(|s| SubTarget::Node(s.to_string())),
            FeedbackKind::Annotation,
            body,
        )
        .expect("a targeted item")
    }

    #[test]
    fn a_recorded_note_is_drawn_back_onto_its_block() {
        // The point of rendering feedback server-side: a reload shows it without
        // a script fetching and rebuilding what Rust already knows how to draw.
        let page = review(
            DOC,
            "demo.md",
            &[note("flow", None, "this branch is wrong")],
            false,
        );
        assert!(page.contains("this branch is wrong"), "{page}");
        assert!(page.contains("on this block"), "{page}");
    }

    #[test]
    fn a_note_aimed_at_a_node_says_which_node() {
        let page = review(
            DOC,
            "d.md",
            &[note("flow", Some("Auth"), "why here")],
            false,
        );
        assert!(page.contains("on Auth"), "{page}");
    }

    #[test]
    fn a_note_on_something_other_than_a_node_still_says_what_it_is_on() {
        // Cell, line and text targets are the designed granularity; the viewer
        // only ever routed nodes, so this arm had nothing exercising it.
        let cell = FeedbackItem::new(
            "flow",
            Some(SubTarget::Cell { row: 2, col: 1 }),
            FeedbackKind::Annotation,
            "wrong figure",
        )
        .expect("a targeted item");
        let page = review(DOC, "d.md", &[cell], false);
        assert!(page.contains("cell:2,1"), "{page}");
    }

    #[test]
    fn a_block_that_takes_comments_gets_a_composer() {
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("action=\"/feedback\""), "{page}");
        assert!(page.contains("name=\"block_id\" value=\"flow\""), "{page}");
    }

    #[test]
    fn the_keyboard_can_close_and_post() {
        // Neither is expressible in markup, which is the test for whether it
        // belongs in the script at all.
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("e.key==='Escape'"), "{page}");
        // Opening puts the cursor in the field. Without it the reviewer clicks
        // twice to start typing, and Escape is inert — the key handler only hears
        // about a composer that holds the focus.
        assert!(page.contains("if(t)t.focus();return;"), "{page}");
        // And the keys work from anywhere inside the composer, not just the field.
        assert!(page.contains("t.closest('.annotate')"), "{page}");
        assert!(
            page.contains("e.key==='Enter'&&(e.ctrlKey||e.metaKey)"),
            "Command-Enter is the same gesture on a Mac: {page}"
        );
        // Closing by key runs the same path as closing by label, so the node
        // target is cleared either way rather than only on a click.
        assert!(page.contains("function shut(d)"), "{page}");
        assert!(page.contains("new Event('change'"), "{page}");
        // And the shortcut is discoverable without a line of chrome for it.
        assert!(page.contains("title=\"Ctrl+Enter\""), "{page}");
    }

    #[test]
    fn closing_the_composer_forgets_the_node_it_was_aimed_at() {
        // Otherwise the block's own Comment button reopens a composer still
        // pointed at the last node clicked — a control that says "comment on this
        // block" and files the note against something else.
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains(".annotate>.toggle"), "{page}");
        assert!(page.contains("f.sub.value=''"), "{page}");
    }

    #[test]
    fn a_question_takes_a_comment_as_well_as_an_answer() {
        // Picking an option says which; a reviewer often wants to say why, or that
        // none of them fit.
        let page = review(DOC, "d.md", &[], false);
        assert!(
            page.contains("f-q1"),
            "the question has no composer: {page}"
        );
    }

    #[test]
    fn an_answer_saves_on_choosing_and_keeps_a_button_for_a_page_with_no_script() {
        // Picking the option *is* the answer, so a button adds nothing — but it is
        // still rendered, because without a script it is the only way to submit.
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("form class=\"answer\""), "{page}");
        assert!(page.contains(">Answer</button>"), "{page}");
        assert!(
            page.contains("b.hidden=true"),
            "the button is not hidden: {page}"
        );
        assert!(page.contains("addEventListener('change'"), "{page}");
    }

    #[test]
    fn the_target_is_shown_on_the_field_rather_than_beside_a_button() {
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("class=\"box\""), "{page}");
        assert!(page.contains(".aim{position:absolute"), "{page}");
        // Full width, so it is not squeezed beside the controls.
        assert!(
            page.contains(".annotate textarea{display:block;width:100%"),
            "{page}"
        );
        // The submit points back at the form so it can share a row with Cancel.
        assert!(page.contains("form=\"f-flow\""), "{page}");
        assert!(page.contains("<form id=\"f-flow\""), "{page}");
    }

    #[test]
    fn cancel_sits_beside_the_button_it_cancels() {
        // Both controls under the field's left edge, the one that posts first.
        // `<details>` could not put them in either order without reordering with
        // flex, because it pins its summary to the front of the markup.
        let page = review(DOC, "d.md", &[], false);
        let cancel = page.find("class=\"cancel\"").expect("a cancel control");
        let post = page
            .find("<button type=\"submit\" form=\"f-flow\"")
            .expect("a submit");
        assert!(post < cancel, "the posting control is not first: {page}");
        assert!(page.contains(".annotate .row{display:flex;justify-content:flex-start"));
        // And the open control is gone while the pane is up, not stacked above it.
        assert!(
            page.contains(".annotate>.toggle:checked~.ask{display:none}"),
            "{page}"
        );
    }

    #[test]
    fn the_composer_is_folded_away_until_it_is_asked_for() {
        // A textarea under every block reads as a form to fill in rather than a
        // document to read, and most blocks are never commented on.
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("<div class=\"annotate\">"), "{page}");
        assert!(page.contains("class=\"ask\""), "{page}");
        // Closed by default: the toggle carries no `checked`.
        assert!(
            !page.contains("class=\"toggle\" id=\"c-flow\" checked"),
            "{page}"
        );
        assert!(page.contains(".annotate>.pane{display:none}"), "{page}");
    }

    #[test]
    fn a_recorded_note_is_not_folded_away_with_the_composer() {
        // What was already said is part of the document; only the way to say more
        // is hidden.
        let page = review(DOC, "d.md", &[note("flow", None, "already said")], false);
        let notes = page.find("already said").expect("the note is on the page");
        let composer = page.find("<div class=\"annotate\">").expect("a composer");
        assert!(
            notes < composer,
            "the note is folded inside the composer: {page}"
        );
    }

    #[test]
    fn a_finished_review_keeps_what_was_said_and_offers_no_way_to_add() {
        let page = review(DOC, "d.md", &[note("flow", None, "kept")], true);
        assert!(page.contains("kept"), "the note vanished: {page}");
        assert!(!page.contains("action=\"/feedback\""), "{page}");
        assert!(!page.contains("action=\"/finish\""), "{page}");
        assert!(page.contains("This review is finished"), "{page}");
    }

    #[test]
    fn finishing_does_not_navigate_either() {
        // The last control that still reloaded, and so the last one that could
        // throw away the reviewer's scroll position.
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("to==='/finish'"), "{page}");
        assert!(page.contains("bar.outerHTML=html"), "{page}");
        // And what no longer applies is taken off the page rather than left inert.
        assert!(page.contains("a.remove()"), "{page}");
        assert!(page.contains("i.disabled=true"), "{page}");
    }

    #[test]
    fn an_open_review_can_be_finished() {
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("action=\"/finish\""), "{page}");
        assert!(!page.contains("This review is finished"), "{page}");
    }

    #[test]
    fn the_script_is_only_what_cannot_be_declared() {
        let page = review(DOC, "d.md", &[], false);
        // Node targeting and the close beacon — and a form for everything else.
        assert!(page.contains("data-id"), "{page}");
        assert!(page.contains("sendBeacon"), "{page}");
        assert!(page.contains("method=\"post\""), "{page}");
        // No renderer, no framework, no fetch of the artifact.
        assert!(!page.contains("/artifact.md"), "{page}");
    }

    #[test]
    fn submitting_a_form_does_not_beacon_the_host_that_the_tab_closed() {
        // Every comment navigates, so `pagehide` fires on each one. Beaconing
        // there armed the host's shutdown and the review died on roughly every
        // other comment, depending on whether the beacon or the next page load
        // reached the request loop first.
        let page = review(DOC, "d.md", &[], false);
        assert!(page.contains("'submit'"), "no submit handler: {page}");
        assert!(page.contains("going=false"), "{page}");
        assert!(page.contains("if(going&&!e.persisted)"), "{page}");
    }

    #[test]
    fn the_name_reaches_the_tab_title() {
        assert!(review(DOC, "plan.md", &[], false).contains("<title>ags · plan.md</title>"));
        assert!(review(DOC, "", &[], false).contains("<title>ags</title>"));
    }

    #[test]
    fn an_answer_is_not_drawn_as_a_note() {
        // It belongs in its question's control, not in the comment list.
        let answer = FeedbackItem::new("q1", None, FeedbackKind::Answer, "SQLite")
            .expect("an answer targets its question");
        let page = review(DOC, "d.md", &[answer], false);
        // Checked against the markup, not the page: `.note-card` is also a CSS
        // rule, and asserting over the whole document would match that instead.
        assert!(!page.contains("<div class=\"note-card\">"), "{page}");
        // It shows up where it belongs — as the option the reviewer picked.
        assert!(page.contains("value=\"SQLite\" checked"), "{page}");
    }

    #[test]
    fn a_redirect_lands_on_the_block_that_was_commented_on() {
        assert_eq!(anchor_for("flow"), "/#block-flow");
    }
}
