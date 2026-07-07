# WebHID 抓包脚本

用于验证 G3 是否和 A7 使用同一套电量协议。

在 MCHOSE Web 页面打开 DevTools Console，在点击连接设备之前执行：

```js
(() => {
  const toHex = (data) => {
    let arr;
    if (data instanceof DataView) {
      arr = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    } else if (data instanceof ArrayBuffer) {
      arr = new Uint8Array(data);
    } else if (ArrayBuffer.isView(data)) {
      arr = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    } else {
      arr = [];
    }
    return [...arr].map(x => x.toString(16).padStart(2, "0")).join(" ");
  };

  if (navigator.hid) {
    const hidProto = Object.getPrototypeOf(navigator.hid);
    const rawRequestDevice = hidProto.requestDevice;
    hidProto.requestDevice = async function(options) {
      console.log("[HID requestDevice options]", options);
      const devices = await rawRequestDevice.call(this, options);
      devices.forEach(device => console.log("[HID DEVICE]", {
        productName: device.productName,
        vendorId: "0x" + device.vendorId.toString(16),
        productId: "0x" + device.productId.toString(16),
        collections: device.collections,
      }));
      return devices;
    };
  }

  if (window.HIDDevice) {
    const p = HIDDevice.prototype;

    const rawSendFeatureReport = p.sendFeatureReport;
    p.sendFeatureReport = function(reportId, data) {
      console.log("[HID sendFeatureReport]", {
        productName: this.productName,
        reportId,
        data: toHex(data),
      });
      return rawSendFeatureReport.call(this, reportId, data);
    };

    const rawReceiveFeatureReport = p.receiveFeatureReport;
    p.receiveFeatureReport = async function(reportId) {
      const ret = await rawReceiveFeatureReport.call(this, reportId);
      console.log("[HID receiveFeatureReport]", {
        productName: this.productName,
        reportId,
        data: toHex(ret),
      });
      return ret;
    };

    const rawAddEventListener = p.addEventListener;
    p.addEventListener = function(type, listener, options) {
      if (type === "inputreport") {
        const wrapped = function(event) {
          console.log("[HID inputreport]", {
            productName: this.productName,
            reportId: event.reportId,
            data: toHex(event.data),
          });
          return listener.call(this, event);
        };
        return rawAddEventListener.call(this, type, wrapped, options);
      }
      return rawAddEventListener.call(this, type, listener, options);
    };
  }

  console.log("[HID logger installed]");
})();
```

需要记录：

```text
productName
vendorId
productId
sendFeatureReport reportId + data
receiveFeatureReport reportId + data
网页显示的电量百分比
网页显示的充电状态
```
