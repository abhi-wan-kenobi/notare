use std::sync::atomic::{AtomicUsize, Ordering};

use cactus::{CompleteOptions, Message, Model, complete};

fn llm_model() -> Model {
    let path = std::env::var("CACTUS_LLM_MODEL")
        .unwrap_or_else(|_| "/tmp/cactus-models/gemma-3-270m-it".into());
    Model::new(&path).unwrap()
}

// cargo test -p cactus --test llm test_complete -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete() {
    let model = llm_model();
    let messages = vec![
        Message::system("Answer in one word only."),
        Message::user("What is 2+2?"),
    ];
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let r = complete(&model, &messages, &options).unwrap();

    assert!(r.total_tokens > 0);
    println!("response: {:?}", r.text);
}

// cargo test -p cactus --test llm test_complete_streaming -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_streaming() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let mut context = model.llm_context(vec![
        Message::system("Answer in one word only."),
        Message::user("What is 2+2?"),
    ]);
    let token_count = AtomicUsize::new(0);

    let r = context
        .complete_streaming(&options, |token| {
            assert!(!token.is_empty());
            token_count.fetch_add(1, Ordering::Relaxed);
            true
        })
        .unwrap();

    assert!(context.messages().len() >= 3);
    println!(
        "streamed {} tokens: {:?}",
        token_count.load(Ordering::Relaxed),
        r.text
    );
}

// cargo test -p cactus --test llm test_complete_streaming_early_stop -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_streaming_early_stop() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(200),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let mut context = model.llm_context(vec![Message::user("Count from 1 to 100")]);
    let token_count = AtomicUsize::new(0);

    let _ = context.complete_streaming(&options, |_token| {
        let n = token_count.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= 3 {
            model.stop();
            return false;
        }
        true
    });

    let final_count = token_count.load(Ordering::Relaxed);
    assert!(
        final_count < 200,
        "should have stopped early, got {final_count} tokens"
    );
    println!("stopped after {final_count} tokens");
}

// cargo test -p cactus --test llm test_complete_multi_turn -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_multi_turn() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(30),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let mut context = model.llm_context(vec![]);
    context.push(Message::user("Say exactly: pineapple"));
    let r1 = context.complete(&options).unwrap();

    context.push(Message::user("What fruit did I just ask you to say?"));
    let r2 = context.complete(&options).unwrap();

    assert!(r1.total_tokens > 0);
    assert!(r2.total_tokens > 0);
    assert_eq!(context.messages().len(), 4);
    assert_eq!(context.messages()[0].role, "user");
    assert_eq!(context.messages()[1].role, "assistant");
    assert_eq!(context.messages()[2].role, "user");
    assert_eq!(context.messages()[3].role, "assistant");
    println!("turn1: {:?}", r1.text);
    println!("turn2: {:?}", r2.text);
}

// cargo test -p cactus --test llm test_complete_reuses_model_statelessly -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_reuses_model_statelessly() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let first = complete(&model, &[Message::user("Say exactly: pineapple")], &options).unwrap();
    let second = complete(&model, &[Message::user("Say exactly: 4")], &options).unwrap();

    assert!(first.total_tokens > 0);
    assert!(second.total_tokens > 0);
    println!("first: {:?}", first.text);
    println!("second: {:?}", second.text);
}

// cargo test -p cactus --test llm test_complete_streaming_reuses_model_statelessly -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_streaming_reuses_model_statelessly() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let first_tokens = AtomicUsize::new(0);
    let first = {
        let mut context = model.llm_context(vec![Message::user("Say exactly: pineapple")]);
        context
            .complete_streaming(&options, |token| {
                first_tokens.fetch_add(1, Ordering::Relaxed);
                !token.is_empty()
            })
            .unwrap()
    };

    let second_tokens = AtomicUsize::new(0);
    let second = {
        let mut context = model.llm_context(vec![Message::user("Say exactly: 4")]);
        context
            .complete_streaming(&options, |token| {
                second_tokens.fetch_add(1, Ordering::Relaxed);
                !token.is_empty()
            })
            .unwrap()
    };

    assert!(first.total_tokens > 0);
    assert!(second.total_tokens > 0);
    assert!(first_tokens.load(Ordering::Relaxed) > 0);
    assert!(second_tokens.load(Ordering::Relaxed) > 0);
    println!("first: {:?}", first.text);
    println!("second: {:?}", second.text);
}

// cargo test -p cactus --test llm test_complete_streaming_early_stop_resets_model -- --ignored --nocapture
#[ignore]
#[test]
fn test_complete_streaming_early_stop_resets_model() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(200),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    {
        let mut context = model.llm_context(vec![Message::user("Count from 1 to 100")]);
        let token_count = AtomicUsize::new(0);
        let _ = context.complete_streaming(&options, |_token| {
            let n = token_count.fetch_add(1, Ordering::Relaxed) + 1;
            if n >= 3 {
                model.stop();
                return false;
            }
            true
        });

        assert!(token_count.load(Ordering::Relaxed) < 200);
    }

    let follow_up = complete(
        &model,
        &[
            Message::system("Answer in one word only."),
            Message::user("What is 2+2?"),
        ],
        &options,
    )
    .unwrap();

    assert!(follow_up.total_tokens > 0);
    println!("follow_up: {:?}", follow_up.text);
}

// cargo test -p cactus --test llm test_llm_context_drop_resets_model -- --ignored --nocapture
#[ignore]
#[test]
fn test_llm_context_drop_resets_model() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    {
        let mut context = model.llm_context(vec![]);
        context.push(Message::user("Say exactly: pineapple"));
        let result = context.complete(&options).unwrap();
        assert!(result.total_tokens > 0);
        assert_eq!(context.messages().len(), 2);
    }

    let fresh = complete(&model, &[Message::user("Say exactly: 4")], &options).unwrap();
    assert!(fresh.total_tokens > 0);
    println!("fresh: {:?}", fresh.text);
}

// cargo test -p cactus --test llm test_llm_context_reset_cache_preserves_history -- --ignored --nocapture
#[ignore]
#[test]
fn test_llm_context_reset_cache_preserves_history() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let mut context = model.llm_context(vec![]);
    context.push(Message::user("Say exactly: pineapple"));
    let first = context.complete(&options).unwrap();

    assert!(first.total_tokens > 0);
    assert_eq!(context.messages().len(), 2);

    context.reset_cache();
    assert_eq!(context.messages().len(), 2);
    assert_eq!(context.messages()[0].role, "user");
    assert_eq!(context.messages()[1].role, "assistant");

    context.push(Message::user("What fruit did I just ask you to say?"));
    let second = context.complete(&options).unwrap();

    assert!(second.total_tokens > 0);
    assert_eq!(context.messages().len(), 4);
    println!("first: {:?}", first.text);
    println!("second: {:?}", second.text);
}

// cargo test -p cactus --test llm test_llm_context_clear_resets_history_and_cache -- --ignored --nocapture
#[ignore]
#[test]
fn test_llm_context_clear_resets_history_and_cache() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let mut context = model.llm_context(vec![]);
    context.push(Message::user("Say exactly: pineapple"));
    let first = context.complete(&options).unwrap();

    assert!(first.total_tokens > 0);
    assert_eq!(context.messages().len(), 2);

    context.clear();
    assert!(context.messages().is_empty());

    context.push(Message::user("Say exactly: 4"));
    let second = context.complete(&options).unwrap();

    assert!(second.total_tokens > 0);
    assert_eq!(context.messages().len(), 2);
    assert_eq!(context.messages()[0].role, "user");
    assert_eq!(context.messages()[1].role, "assistant");
    println!("second: {:?}", second.text);
}

// cargo test -p cactus --test llm test_llm_context_invalid_request_preserves_history -- --ignored --nocapture
#[ignore]
#[test]
fn test_llm_context_invalid_request_preserves_history() {
    let model = llm_model();
    let options = CompleteOptions {
        max_tokens: Some(20),
        temperature: Some(0.0),
        confidence_threshold: Some(0.0),
        ..Default::default()
    };

    let mut context = model.llm_context(vec![
        Message::system("You are a helpful assistant."),
        Message::user(vec![hypr_llm_types::MessagePart::image_url(
            "https://example.com/test.png",
        )]),
    ]);

    let error = context.complete(&options).unwrap_err();

    assert!(matches!(error, cactus::Error::InvalidRequest(_)));
    assert_eq!(context.messages().len(), 2);
    assert_eq!(context.messages()[0].role, "system");
    assert_eq!(context.messages()[1].role, "user");
}
