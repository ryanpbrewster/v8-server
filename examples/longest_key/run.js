let key = "";
let longest = "";
while (key !== undefined) {
  if (key.length > longest.length) longest = key;
  key = api.next(key);
}
longest;
