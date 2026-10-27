# Library Ask finds Meetings by expanded keywords, not embeddings

A **Library Ask** must find the relevant Segments across every Meeting before a Language model can answer. We find them with the Library search index we already have: the Language model first rewrites the question into keywords and synonyms (in the Meeting languages the Library uses), then the full-text index ranks Segments by BM25, each hit brings its neighbouring Segments, and evidence is added until the Provider's token budget is spent. There is no cap on the number of Meetings; the budget decides. If the rewrite fails, retrieval runs on the question's own keywords.

This keeps everything rebuildable from the Library with no new model download or second index, works the same on Ollama as on cloud Language models, and handles most paraphrases ("budget" finding "we can spend 20k") that plain keywords miss.

## Considered Options

- **Keywords only:** cheapest, but misses paraphrases and cross-language wording, which matters for non-English Meetings.
- **Local embeddings:** best recall, but adds an embedding model download, a vector index to rebuild from the Library, and slow first indexing on CPU. Revisit if evaluation shows poor recall.

## Consequences

- The question (not Transcript text) reaches a cloud Language model before the user confirms which Meetings to send; Transcript excerpts are sent only after that confirmation.
- On Ollama's 8k context a Library Ask reads far less than on a cloud model; the UI reports how many Meetings were read.
