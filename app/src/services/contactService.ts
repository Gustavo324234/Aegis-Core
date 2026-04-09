import * as Contacts from 'expo-contacts';

export async function searchContacts(name: string) {
  const { status } = await Contacts.requestPermissionsAsync();
  if (status !== 'granted') {
    throw new Error('Permission to access contacts was denied');
  }

  const { data } = await Contacts.getContactsAsync({
    name,
    fields: [Contacts.Fields.Emails, Contacts.Fields.PhoneNumbers],
  });

  if (data.length > 0) {
    return data.map(contact => ({
      id: contact.id,
      name: contact.name,
      phones: contact.phoneNumbers?.map(p => p.number),
      emails: contact.emails?.map(e => e.email),
    }));
  }

  return [];
}

export async function getAllContacts() {
  const { status } = await Contacts.requestPermissionsAsync();
  if (status !== 'granted') {
    throw new Error('Permission to access contacts was denied');
  }

  const { data } = await Contacts.getContactsAsync({
    fields: [Contacts.Fields.Name, Contacts.Fields.PhoneNumbers],
  });

  return data.map(c => ({ name: c.name, phones: c.phoneNumbers?.map(p => p.number) }));
}
