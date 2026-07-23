import json, numpy as np, onnxruntime as ort
from tokenizers import Tokenizer

SP = "/tmp/claude-1000/-home-skywalker-vaults-personal/8b2d280b-2bdf-4860-9488-48302cd50d01/scratchpad"
QP = "task: search result | query: "
DP = "title: none | text: "

tok = Tokenizer.from_file(f"{SP}/egemma/tokenizer.json")
sess = ort.InferenceSession(
    f"{SP}/egemma/onnx/model_quantized.onnx", providers=["CPUExecutionProvider"]
)


def embed(texts):
    encs = [tok.encode(t) for t in texts]
    maxlen = max(len(e.ids) for e in encs)
    ids = np.zeros((len(encs), maxlen), dtype=np.int64)
    mask = np.zeros((len(encs), maxlen), dtype=np.int64)
    for i, e in enumerate(encs):
        ids[i, : len(e.ids)] = e.ids
        mask[i, : len(e.ids)] = e.attention_mask
    out = sess.run(["sentence_embedding"], {"input_ids": ids, "attention_mask": mask})[
        0
    ]
    return out


# sanity: is sentence_embedding L2-normalized? and what do token ids look like
e = tok.encode(DP + "hello world")
print("SAMPLE_TOKENS:", e.ids[:6], "...", e.ids[-3:], "n=", len(e.ids))
v = embed([DP + "hello world"])[0]
print("NORM_768:", float(np.linalg.norm(v)))

sents = [
    "The quarterly revenue exceeded expectations by twelve percent.",
    "Please schedule the follow-up meeting for next Tuesday afternoon.",
    "Our new logistics platform reduces last-mile delivery costs.",
    "The patient reported mild headaches after starting the medication.",
    "Photosynthesis converts sunlight into chemical energy in plants.",
    "The espresso machine needs descaling every three months.",
    "Rust's borrow checker prevents data races at compile time.",
    "The hiking trail closes during the monsoon season.",
    "Interest rates were held steady by the central bank.",
    "She adopted a rescue greyhound from the local shelter.",
    "The bridge retrofit will take eighteen months to complete.",
    "Quantum computers use qubits instead of classical bits.",
    "Add two cups of flour and knead for ten minutes.",
    "The documentary explores coral bleaching in the Pacific.",
    "His flight was delayed by a storm over the Atlantic.",
    "The startup pivoted from consumer apps to enterprise software.",
    "Regular exercise improves cardiovascular health markers.",
    "The museum's new wing features contemporary sculpture.",
    "We migrated the database to a managed cloud service.",
    "The choir rehearses in the old chapel on Thursdays.",
]
docs10 = sents[:10]
queries5 = [
    "how did the company do on revenue this quarter",
    "set up a meeting next week",
    "cutting delivery costs in logistics",
    "side effects of the new medicine",
    "how do plants turn light into energy",
]

doc_vecs = embed([DP + s for s in sents])
q_vecs = embed([QP + q for q in queries5])
# prefix-trap vectors: same text embedded as query vs doc must differ
trap_q = embed([QP + sents[0]])[0]
trap_d = embed([DP + sents[0]])[0]
print(
    "TRAP_COSINE(q vs d same text):",
    float(trap_q @ trap_d / (np.linalg.norm(trap_q) * np.linalg.norm(trap_d))),
)


def trunc512(v):
    t = v[:512]
    return t / np.linalg.norm(t)


rank = {}
for qi, q in enumerate(queries5):
    qv = trunc512(q_vecs[qi])
    sims = [float(qv @ trunc512(doc_vecs[di])) for di in range(10)]
    order = sorted(range(10), key=lambda d: -sims[d])
    rank[q] = {"top3": order[:3], "sims": sims}
    print("QUERY", qi, "top3:", order[:3])

fx = {
    "model": "onnx-community/embeddinggemma-300m-ONNX @ model_quantized",
    "query_prefix": QP,
    "doc_prefix": DP,
    "dim_full": 768,
    "dim_truncated": 512,
    "sentences": sents,
    "doc_embeddings_768": [[float(x) for x in v] for v in doc_vecs],
    "queries": queries5,
    "query_embeddings_768": [[float(x) for x in v] for v in q_vecs],
    "rank_top3": {q: rank[q]["top3"] for q in queries5},
}
with open(f"{SP}/reference_embeddings.json", "w") as f:
    json.dump(fx, f)
print(
    "FIXTURE_WRITTEN",
    len(sents),
    "sents;",
    "bytes:",
    __import__("os").path.getsize(f"{SP}/reference_embeddings.json"),
)
