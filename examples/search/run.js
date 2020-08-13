const needle = "qu";

let key = "";
while (key !== undefined) {
  if (key.includes(needle)) break;
  key = api.next(key);
}
key
