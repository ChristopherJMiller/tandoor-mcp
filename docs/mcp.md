# MCP tool behavior

## Keyword listing

`get_keywords` returns every recipe keyword/tag visible to the configured
Tandoor user. Tandoor exposes keywords as a paginated Django REST Framework
collection; the client requests pages of up to 100 keywords and follows each
`next` link before producing the MCP result.

Pagination links are resolved onto `TANDOOR_BASE_URL` before the bearer token is
attached. This supports Tandoor deployments behind a reverse proxy without
forwarding credentials to a host supplied in an API response.

Collection traversal stops with an error after 100 pages. This safety limit
prevents malformed or cyclic pagination links from creating an unbounded
request loop. A successful response has:

- `keywords`: the complete collected list;
- `total_count`: Tandoor's count for the collection.

Recipe discovery can use the returned tag metadata to identify how recipes are
categorized. Access remains limited to the active Tandoor user's space and
permissions.
