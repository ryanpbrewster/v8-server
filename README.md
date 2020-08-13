# Flexible APIs

Here's a common story in my development experience:
  1. Build a simple, elegant API.
  2. Build some features on top of that API.
  3. Realize that for some reason (efficiency or atomicity, usually), I need to add a "special purpose" endpoint.
  4. Add the special purpose endpoint.
  5. Go to step 2.

As an example, consider a simple key-value store API:
```
service KeyValueStore {
  rpc Get(GetRequest) returns (KeyValue) {}
  rpc Set(KeyValue) returns (google.protobuf.Empty) {}
}

message KeyValue {
  string key = 1;
  string value = 2;
}

message GetRequest {
  string key = 1;
}
```

which might have a TypeScript API that looks something like
```
interface Client {
  async get(key: string): Promise<string>
  async set(key: string, value: string): Promise<void>
}
```

For a while, all is well. Then, new requirements. We may need to be able to set
multiple values at the same time (atomicity) --- perhaps issuing multiple
requests is too expensive, or we really want to avoid partial updates. We might
introduce a batch set API.
```
  rpc BatchSet(BatchSetRequest) returns (google.protobuf.Empty) {}

message BatchSetRequest {
  repeated KeyValue pairs = 1;
}
```

Again, time passes. Now we need a new feature --- perhaps we want to set a value,
but only if it doesn't already have a value (i.e., create/insert semantics). One
option here is to directly add a new endpoint
```
  rpc Create(KeyValue) returns (google.protobuf.Empty) {}
```
which has the same inputs and outputs as the existing `Set` method, but with
different semantics.

Another option is to extend the existing endpoint
```
  rpc Set(SetRequest) returns (google.protobuf.Empty) {}

method SetRequest {
  string key = 1;
  string value = 2;
  Precondition = 3;
}
enum Precondition {
  KEY_ABSENT = 0;
  KEY_PRESENT = 1;
}
```
so that the client can choose put, insert, or update semantics.

In my experience, usually this story repeats until the exposed API is quite complex.
Worse still, it's usually the case that old clients will have hacked around the
API while waiting for some special-purpose endpoint.

This repo is an exploration of one way to take a fairly simple API and give clients
the ability to extend it themselves.
