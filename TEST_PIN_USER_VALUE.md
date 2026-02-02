# Testing Pin User Value Feature

## Quick Test Steps

1. **Start the application**
   ```bash
   npm run tauri dev
   ```

2. **Create or open a project with an Event**

3. **Add a Print node**
   - Drag a Print node from the node palette onto the canvas
   - Connect it to the Event's "Exec" output

4. **Set a value on the Print node's Value pin**
   - Look for the "Value" input pin on the Print node
   - You should see a text input widget next to it (since it's not connected)
   - Click on the input and type a value, e.g., "Hello World"
   - Press Enter or click away to save

5. **Check the console logs**
   - Open the browser DevTools (F12)
   - You should see logs like:
     ```
     [PinInput] Saving value: { subgraphId: "...", nodeId: "...", pinId: "...", value: "Hello World", pinType: "any" }
     [PinInput] Value saved successfully to backend
     [PinInput] Updated frontend store for input pin: ...
     [PinInput] Frontend store updated successfully
     ```

6. **Execute the Event**
   - Click the "Execute" button (play icon)
   - Check the output in the toast notification or console

7. **Expected Result**
   - The Print node should output: "Hello World"
   - NOT "null" as before

8. **Save and reload test**
   - Save the project (Ctrl+S)
   - Close and reopen the project
   - Execute again - the value should still be there

## Testing Different Pin Types

### Integer Pin
1. Add a node with an integer input (e.g., Math Add node)
2. Enter a number like `42`
3. Execute and verify the value is used

### Float Pin
1. Add a node with a float input
2. Enter a decimal like `3.14`
3. Execute and verify the value is used

### Boolean Pin
1. Add a node with a boolean input
2. Check/uncheck the checkbox
3. Execute and verify the value is used

### String Pin
1. Add a node with a string input
2. Enter text like "test"
3. Execute and verify the value is used

## Troubleshooting

### Value still shows as null
- Check browser console for error messages
- Verify the backend logs show the value being saved
- Make sure you pressed Enter or clicked away after entering the value
- Try refreshing the page and re-entering the value

### Input widget not showing
- Make sure the pin is NOT connected to another pin
- Verify the pin is an input pin (on the left side of the node)
- Check that the pin type is not "exec"

### Value not persisting after reload
- Make sure you saved the project after setting the value
- Check the project JSON file - it should have a "userValue" field in the pin data
- Verify the file was actually saved to disk

## Backend Logs to Check

In the backend console, you should see:
```
[update_pin_user_value] subgraph_id=..., node_id=..., pin_id=..., value=...
[update_pin_user_value] Found node: print
[update_pin_user_value] Found input pin: Value, old user_value: None
[update_pin_user_value] Updated input pin user_value to: Some(String("Hello World"))
[update_pin_user_value] Successfully updated pin value
```

During execution:
```
[get_pin_value] Using user value for pin '...': String("Hello World")
[Print] Hello World
```

## Success Criteria

✅ Input widget appears on unconnected data pins
✅ Value can be entered and saved
✅ Backend logs show value being saved
✅ Frontend store is updated
✅ Execution uses the entered value (not null)
✅ Value persists after save/reload
✅ Different pin types (int, float, bool, string, any) all work correctly
